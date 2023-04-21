use anyhow::{bail, Context, Result};
use itm::{
    serial, Decoder, DecoderOptions, LocalTimestampOptions, TimestampsConfiguration, TracePacket,
};
use std::fs::File;
use std::path::PathBuf;
use std::str;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    about = "An ITM/DWT packet protocol decoder, as specified in the ARMv7-M architecture reference manual, Appendix D4. See <https://developer.arm.com/documentation/ddi0403/ed/>. Report bugs and request features at <https://github.com/rust-embedded/itm>."
)]
struct Opt {
    #[structopt(long = "--ignore-eof")]
    ignore_eof: bool,

    #[structopt(long = "--timestamps", requires("freq"))]
    timestamps: bool,

    #[structopt(long = "--itm-prescaler")]
    prescaler: Option<u8>,

    #[structopt(long = "--itm-freq", name = "freq")]
    freq: Option<u32>,

    #[structopt(long = "--expect-malformed")]
    expect_malformed: bool,

    #[structopt(name = "FILE", parse(from_os_str), help = "Raw trace input file.")]
    file: PathBuf,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let file = File::open(&opt.file).context("failed to open file")?;
    if let Some(freq) = opt.freq {
        serial::configure(&file, freq)?;
    }

    let decoder = Decoder::<File>::new(
        file,
        DecoderOptions {
            ignore_eof: opt.ignore_eof,
        },
    );

    match opt {
        Opt {
            timestamps: true,
            prescaler,
            freq: Some(freq),
            expect_malformed,
            ..
        } => {
            for packets in decoder.timestamps(TimestampsConfiguration {
                clock_frequency: freq,
                lts_prescaler: match prescaler {
                    None | Some(1) => LocalTimestampOptions::Enabled,
                    Some(4) => LocalTimestampOptions::EnabledDiv4,
                    Some(16) => LocalTimestampOptions::EnabledDiv16,
                    Some(64) => LocalTimestampOptions::EnabledDiv64,
                    Some(n) => bail!(
                        "{} is not a valid prescaler; valid prescalers are: 4, 16, 64.",
                        n
                    ),
                },
                expect_malformed,
            }) {
                match packets {
                    Err(e) => return Err(e).context("Decoder error"),
                    Ok(packets) => println!("{:?}", packets),
                }
            }
        }
        _ => {
            let mut log_line: Vec<u8> = Vec::new();
            for packet in decoder.singles() {
                match packet {
                    Err(e) => return Err(e).context("Decoder error"),
                    Ok(TracePacket::Instrumentation { port, payload }) => {
                        if payload.len() == 1 && payload[0] == 10 {
                            match str::from_utf8(&log_line) {
                                Ok(s) => println!("{port}\t{s}"),
                                Err(e) => eprintln!("{e}"),
                            }
                            log_line.clear();
                        } else {
                            for c in payload {
                                log_line.push(c);
                            }
                        }
                    }
                    Ok(packet) => println!("{:?}", packet),
                }
            }
        }
    }

    Ok(())
}
