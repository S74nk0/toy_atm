use clap::Parser;
use csv::Trim;
use std::{fs::File, path::PathBuf};
use toy_atm::accounting::{atm::Atm, transaction::Transaction};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    pub in_file_path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut atm = Atm::default();

    // handle input
    let input_file = File::open(args.in_file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .trim(Trim::All)
        .flexible(true)
        .from_reader(input_file);
    for tx in rdr.deserialize::<Transaction>().flatten() {
        _ = atm.handle_transaction(tx);
    }

    // print output
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let mut csv_writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(handle);
    for cbs in atm.accounts() {
        csv_writer.serialize(cbs)?
    }

    Ok(())
}
