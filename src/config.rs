use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub db_path: PathBuf,
    pub listen_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("D:\\Prototypes\\Project_3\\data\\language_graph.sqlite"),
            listen_port: 8080,
        }
    }
}
