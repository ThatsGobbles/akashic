use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use failure::Error;

use metadata::types::MetaBlock;
use metadata::location::MetaLocation;
use metadata::reader::MetaReader;
use metadata::plexer::MetaPlexer;

pub struct MetaProcessor;

impl MetaProcessor {
    pub fn process_meta_file<MR, P>(
        meta_path: P,
        meta_location: MetaLocation,
    ) -> Result<HashMap<PathBuf, MetaBlock>, Error>
    where
        MR: MetaReader,
        P: AsRef<Path>,
    {
        let meta_structure = MR::from_file(&meta_path, meta_location)?;

        let item_paths = meta_location.get_item_paths(&meta_path)?;

        let meta_plexed = MetaPlexer::plex(meta_structure, item_paths);

        Ok(meta_plexed)
    }

    pub fn process_meta_file_cached<MR, P>(
        meta_path: P,
        meta_location: MetaLocation,
        cache: &mut HashMap<PathBuf, HashMap<PathBuf, MetaBlock>>,
        force: bool,
    ) -> Result<&HashMap<PathBuf, MetaBlock>, Error>
    where
        MR: MetaReader,
        P: AsRef<Path>,
    {
        let meta_path = meta_path.as_ref();

        if force {
            cache.remove(meta_path);
        }

        let meta_file_results = match cache.entry(meta_path.to_owned()) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(Self::process_meta_file::<MR, _>(meta_path, meta_location)?),
        };

        Ok(meta_file_results)
    }

    pub fn process_item_file_cached<MR, P>(
        item_path: P,
        meta_location: MetaLocation,
        cache: &mut HashMap<PathBuf, HashMap<PathBuf, MetaBlock>>,
        force: bool,
    ) -> Result<&MetaBlock, Error>
    where
        MR: MetaReader,
        P: AsRef<Path>,
    {
        let meta_path = meta_location.get_meta_path(&item_path)?;

        let processed_meta_file = Self::process_meta_file_cached::<MR, _>(&meta_path, meta_location, cache, force)?;
        processed_meta_file.get(item_path.as_ref())
            .ok_or(bail!("item path not found in processed metadata: \"{}\"", item_path.as_ref().to_string_lossy()))
    }
}

#[cfg(test)]
mod tests {
    use super::MetaProcessor;

    use failure::Error;

    use metadata::reader::MetaReader;
    use metadata::reader::yaml::YamlMetaReader;
    use metadata::location::MetaLocation;
    use metadata::structure::MetaStructure;

    use test_util::create_temp_media_test_dir;

    // struct TestMetaReader;

    // impl MetaReader for TestMetaReader {
    //     fn from_str<S: AsRef<str>>(_: S, mt: MetaLocation) -> Result<MetaStructure, Error> {
    //         Meta
    //     }

    //     fn from_file<P: AsRef<Path>>(p: P, mt: MetaLocation) -> Result<MetaStructure, Error> {
    //         let p = p.as_ref();
    //         let mut f = File::open(p)?;

    //         let mut buffer = String::new();
    //         f.read_to_string(&mut buffer)?;

    //         Self::from_str(buffer, mt)
    //     }
    // }

    #[test]
    fn test_process_meta_file() {
        let temp_dir = create_temp_media_test_dir("test_process_meta_file");

        let result = MetaProcessor::process_meta_file::<YamlMetaReader, _>(temp_dir.path().join("self.yml"), MetaLocation::Contains);

        println!("{:?}", result);
    }
}
