use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[clap(long, default_value_t = default_config_path())]
    pub config_path: String,
}

impl Args {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

fn default_config_path() -> String {
    "hyperrpc.toml".to_owned()
}

