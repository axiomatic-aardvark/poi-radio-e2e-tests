pub mod checks;
mod graphql;
pub mod setup;

use checks::test_poi_ok::run_poi_ok;
use clap::Parser;
use graphcast_sdk::init_tracing;
use setup::basic_instance::run_basic_instance;
use std::str::FromStr;
use tracing::{error, info};

use crate::{checks::{
    correct_filtering_default_topics::run_correct_filtering_default_topics,
    correct_filtering_different_topics::run_correct_filtering_different_topics,
    test_num_messages::run_num_messages, invalid_sender::run_invalid_sender,
}};

#[derive(Clone, Debug)]
enum Instance {
    BasicInstance,
}

#[derive(Clone, Debug)]
enum Check {
    PoiOk,
    NumMessages,
    CorrectFilteringDefaultTopics,
    CorrectFilteringDifferentTopics,
    InvalidSender
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    instance: Option<String>,
    #[arg(short, long)]
    check: Option<String>,
    #[arg(long)]
    count: Option<u32>,
}

impl FromStr for Instance {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "basic" => Ok(Instance::BasicInstance),
            _ => Err(format!("Invalid instance type: {s}")),
        }
    }
}

impl FromStr for Check {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "poi_ok" => Ok(Check::PoiOk),
            "num_messages" => Ok(Check::NumMessages),
            "correct_filtering_default_topics" => Ok(Check::CorrectFilteringDefaultTopics),
            "correct_filtering_different_topics" => Ok(Check::CorrectFilteringDifferentTopics),
            "invalid_sender" => Ok(Check::InvalidSender),
            _ => Err(format!("Invalid check type: {s}")),
        }
    }
}

#[tokio::main]
pub async fn main() {
    init_tracing().expect("Could not set up global default subscriber");
    let args = Args::parse();

    if let Some(instance) = &args.instance {
        match Instance::from_str(instance) {
            Ok(Instance::BasicInstance) => {
                info!("Starting basic instance");

                std::thread::spawn(|| {
                    run_basic_instance();
                })
                .join()
                .expect("Thread panicked")
            }
            Err(err) => error!("Error: {}", err),
        }
    }

    if let Some(check) = &args.check {
        match Check::from_str(check) {
            Ok(Check::PoiOk) => std::thread::spawn(|| {
                info!("Starting poi_ok check");
                run_poi_ok();
            })
            .join()
            .expect("Thread panicked"),
            Ok(Check::NumMessages) => {
                let count = args.count;

                let count = count.unwrap_or_else(|| {
                    error!("No 'count' argument provided, defaulting to '5'.");
                    5
                });

                std::thread::spawn(move || {
                    info!("Starting num_messages check");
                    run_num_messages(count);
                })
                .join()
                .expect("Thread panicked");
            }
            Ok(Check::CorrectFilteringDefaultTopics) => std::thread::spawn(|| {
                info!("Starting correct_filtering_default_topics check");
                run_correct_filtering_default_topics();
            })
            .join()
            .expect("Thread panicked"),
            Ok(Check::CorrectFilteringDifferentTopics) => std::thread::spawn(|| {
                info!("Starting correct_filtering_different_topics check");
                run_correct_filtering_different_topics();
            })
            .join()
            .expect("Thread panicked"),
            Ok(Check::InvalidSender) => std::thread::spawn(|| {
                info!("Starting invalid_sender check");
                run_invalid_sender();
            })
            .join()
            .expect("Thread panicked"),
            Err(err) => error!("Error: {}", err),
        }
    }
}
