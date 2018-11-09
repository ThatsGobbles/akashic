//! This is intended to be the main public API of the library.

use std::path::Path;
use std::collections::BTreeMap;

use config::Config;
use metadata::types::MetaBlock;
use metadata::processor::MetaProcessor;
use metadata::processor::Error as ProcessorError;
use config::agg_method::AggMethod;

#[derive(Debug)]
pub enum Error {
    CannotProcessMetadata(ProcessorError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::CannotProcessMetadata(ref err) => write!(f, "cannot process metadata: {}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::CannotProcessMetadata(ref err) => Some(err),
        }
    }
}

pub struct MetaFinalizer;

impl MetaFinalizer {
    pub fn get_metadata<P: AsRef<Path>>(item_path: P) -> Result<MetaBlock, Error> {
        // Use a default configuration and no aggregations.
        let config = Config::default();
        let agg_methods = BTreeMap::new();

        Self::get_metadata_with_config_and_aggs(item_path, &config, &agg_methods)
    }

    pub fn get_metadata_with_config<P: AsRef<Path>>(item_path: P, config: &Config) -> Result<MetaBlock, Error> {
        let agg_methods = BTreeMap::new();

        Self::get_metadata_with_config_and_aggs(item_path, config, &agg_methods)
    }

    pub fn get_metadata_with_aggs<P: AsRef<Path>>(item_path: P, agg_methods: &BTreeMap<String, AggMethod>) -> Result<MetaBlock, Error> {
        let config = Config::default();

        Self::get_metadata_with_config_and_aggs(item_path, &config, agg_methods)
    }

    pub fn get_metadata_with_config_and_aggs<P: AsRef<Path>>(
        item_path: P,
        config: &Config,
        agg_methods: &BTreeMap<String, AggMethod>,
    ) -> Result<MetaBlock, Error>
    {
        let mb = MetaProcessor::process_item_file(
            item_path,
            config.meta_format,
            &config.selection,
            config.sort_order,
        ).map_err(Error::CannotProcessMetadata)?;

        Ok(mb)
    }
}
