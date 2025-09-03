//! The file containing asset group functions to access data managed by the resource packing system.
//!
//! # The asset system of let-engine
//!
//! This feature can be enabled with the `asset_system` feature flag.
//!
//! This system allows you to select an assets folder, packs the contents using the given settings including compression,
//! drops the packed asset files to a specified location relative to the binary output path and easily allows you to
//! access the asset data using the `asset` function provided here.
//!
//! ## Basic usage
//!
//! - Create an assets folder usually next to src
//! - Make a config.toml and configure it to your needs
//! - Specify groups where you place your assets
//! - When you want your assets packed, ready to be used, compile the binary using the `ASSETS_DIR` flag set to your asset folder path.
//! - Access the data using the `asset` function with the asset root relative resource path.
//! - Inspect lower ram usage in a system monitor application.
//!
//! ## Groups
//!
//! "Groups" are user defined asset categories that you can give their own settings like compression or naming.
//!
//! ## Asset directory layout
//!
//! assets/
//! - config.toml
//! - group-1
//!   - config.toml (optional)
//! - group-2
//!   - config.toml (optional)
//!
//! ...
//!
//! An example to a config file is located in the examples folder titled `example-resource-config.toml`
//!
//! ## Compression algorithms
//!
//! Each compression algorithm can be accessed by toggling a feature with the same name. It also allows you to select that
//! compression method in the asset config.
//!
//! - deflate - balanced speed and size / known for zip and gzip
//! - bzip2 - slower but more effectively compressed
//! - zstd - very fast decompression
//! - lzma - high compression but slower
//! - lz4 - very fast but low compression ratio
//!
//! The config example also specifies the highest compression ratio of each algorithm.
//!
//! ## Config file settings
//!
//! config.toml
//!
//! - `max_size` - bytes
//!   - The resource size threshold before it gets split to another file.
//!     Larger sizes sometimes mean more memory usage.
//! - `compression` - string
//!   - The compression chosen for the specific group. Using compression requires one of the compression features to be enabled.
//!     The string names can be found above at Compression algorithms.
//! - `compression_level` - integer
//!   - The level at which it gets compressed if compression is enabled.
//!     Each compression algorithm has their own highest and lowest values. Giving any value above the maximum for the specific algorithm clamps it to their
//!     own max. From most compressed to fastest they follow:
//!     - deflate - 0 - 9
//!     - bzip2 - 0 - 9
//!     - zstd - 1 - 22 where giving 0 is the same as giving a 3
//!     - lzma - 0 - 9
//!     - lz4 - 0 - 16
//! - `naming` - string
//!   - The way the files get named. Any `%g`'s get automatically converted to the group name and any `%i`'s to the group index,
//!     if multiple files get produced from the `max_size`. Please request more ways to name files if you want to.
//! - `exclude` - array of paths
//!   - Excludes any files relative to the group root to be included in the packaged files.
//! - `output` - path
//!   - The binary relative path of where the resulting assets should be stored. Can be made to `.` if the assets should be stored right next to the binary
//!     or `assets` or if you have a more complex resource system `resources/assets`. The build process will automatically generate those paths.
//! - `groups` - table of keys containing paths
//!   - Defines the assets folder relative paths to all the groups. Each group also gets a function defined here. Run `cargo doc` and find the documentation here if
//!     you want to see them here. This key does not do anything for a group config.
//!
//! ## Resulting asset files
//!
//! All generated asset files are `bincode` serialized and compressed `HashMap`s with strings as keys holding the asset path relative asset path and as values
//! the data as `Vec`s of `u8`like this:
//! `HashMap<String, Vec<u8>>`
//!
//! ## Usual group examples
//!
//! You can make a group for each asset type like `texture`, `sound` or `model`.
//! For each one of the groups you can select your custom settings using the `config.toml`.
//!
//! The way you would access the data contained in those groups you would run
//!
//! `asset("textures/environment/stone.png")`
//!
//! `asset("sounds/monsters/zombie/growl.oga")`
//!
//! and get the data to be used in the game engine.

#[allow(unused_imports)]
use std::{
    fs,
    io::{Read, Write},
    sync::{Arc, LazyLock},
};

use foldhash::HashMap;

use anyhow::{Result, anyhow};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Every resource path to the disk path where the asset is located with the compression algorithm.
static MAP: LazyLock<HashMap<String, (std::path::PathBuf, Compression)>> = LazyLock::new(|| {
    let data = include_bytes!(concat!(env!("OUT_DIR"), "/map_data"));
    bincode::serde::decode_from_slice(data, bincode::config::standard())
        .unwrap_or_default()
        .0
});

/// The compression algorithm used for the resources.
///
/// Each field requires a feature to be enabled.
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    /// No compression at all.
    #[default]
    None,
    /// Deflate or Flate compression algorithm provided by the `flate2` library.
    ///
    /// Requires the `deflate` feature to be enabled.
    #[cfg(feature = "deflate")]
    Deflate,
    /// Burrows-Wheeler Transform compression algorithm provided by the `bzip2` library.
    ///
    /// Requires the `bzip2` feature to be enabled.
    #[cfg(feature = "bzip2")]
    Bwt,
    /// Z Standard compression algorithm provided by the `zstd` library.
    ///
    /// Requires the `zstd` feature to be enabled.
    #[cfg(feature = "zstd")]
    Zstd,
    /// LZMA compression algorithm provided by the `xz2` library.
    ///
    /// Requires the `lzma` feature to be enabled.
    #[cfg(feature = "lzma")]
    Lzma,
    /// LZ4 compression algorithm provided by the `lz4` library.
    ///
    /// Requires the `lz4` feature to be enabled.
    #[cfg(feature = "lz4")]
    Lz4,
}

impl Compression {
    /// Decompresses the given buffer using the compression algorithm of self.
    pub fn decompress(&self, buffer: &[u8]) -> Result<Vec<u8>> {
        #[allow(unused_assignments)]
        let mut decompressed = vec![];
        match self {
            Compression::None => {
                decompressed = buffer.to_vec();
            }
            #[cfg(feature = "deflate")]
            Compression::Deflate => {
                let mut decoder = flate2::write::GzDecoder::new(&mut decompressed);
                decoder.write_all(buffer)?;
                decoder.finish()?;
            }
            #[cfg(feature = "bzip2")]
            Compression::Bwt => {
                let mut decoder = bzip2::write::BzDecoder::new(&mut decompressed);
                decoder.write_all(buffer)?;
                decoder.finish()?.flush()?;
            }
            #[cfg(feature = "zstd")]
            Compression::Zstd => {
                decompressed = zstd::decode_all(buffer)?;
            }
            #[cfg(feature = "lzma")]
            Compression::Lzma => {
                let mut decoder = xz2::write::XzDecoder::new(&mut decompressed);
                decoder.write_all(buffer)?;
                decoder.finish()?.flush()?;
            }
            #[cfg(feature = "lz4")]
            Compression::Lz4 => {
                let mut decoder = lz4::Decoder::new(buffer)?;
                decoder.read_to_end(&mut decompressed)?;
            }
        }
        Ok(decompressed)
    }
}

/// An error that can occur by trying to load an asset using the asset system.
#[derive(thiserror::Error, Debug)]
pub enum AssetError {
    /// The asset you are trying to open does not exist.
    #[error("This asset does not exist.")]
    NotListed,
    /// The compression format this asset has is not supported by the game.
    ///
    /// This can be solved by compiling the game using a feature flag with the same compression as the build script.
    #[error("The format given by this file is not recognized: {0:?}")]
    UnsupportedFormat(anyhow::Error),
    /// The asset file can not be read.
    #[error("There was a problem opening this asset file: {0:?}")]
    Io(std::io::Error),
}

/// Returns an asset from the cache and loads and unpacks it, if it is not loaded yet. May take a while for some objects to get returned.
///
/// This function can also be called to precache assets here.
///
/// It takes the asset directory relative path to a resource found inside and returns it.
pub fn asset(path: &str) -> Result<Arc<[u8]>, AssetError> {
    CACHE.get_or_load(path)
}

/// Clears the asset cache for unused keys and removes them. When calling the `asset` function for an unloaded asset it takes the same time
/// as it did first again.
pub fn clear_cache() {
    CACHE.clear();
}

/// The asset cache holding all currently loaded assets.
#[derive(Debug)]
struct Cache {
    map: RwLock<HashMap<String, Arc<[u8]>>>,
}

impl Cache {
    /// Returns the data to an asset using the asset directory relative path to the asset you are trying to access.
    pub fn get_or_load(&self, key: &str) -> Result<Arc<[u8]>, AssetError> {
        // Return data if it is listed in the cache
        if let Some(data) = self.map.read().get(key) {
            return Ok(data.clone());
        }

        // else load it into the cache.

        // Error when the key does not exist,
        let Some((file_path, compression)) = MAP.get(key) else {
            return Err(AssetError::NotListed);
        };

        // Path where the key data is stored:
        let asset_path = {
            let application_path = std::env::current_exe().map_err(AssetError::Io)?;
            let application_path = application_path.parent().unwrap();
            application_path.join(file_path)
        };

        // Decompressed and deserialized HashMap of keys and data
        let map: HashMap<String, Vec<u8>> = {
            // Read from disk,
            let data = fs::read(asset_path).map_err(AssetError::Io)?;
            // Uncompress if it has compression or return an error if it does not have a supported format.
            let data = compression
                .decompress(&data)
                .map_err(AssetError::UnsupportedFormat)?;

            bincode::serde::decode_from_slice(&data, bincode::config::standard())
                .map_err(|x| AssetError::UnsupportedFormat(x.into()))?
                .0
        };

        let mut result: Option<Arc<[u8]>> = None;

        // Load to cache in a way quickly accessable.
        for (key2, value) in map {
            let mut write = self.map.write();

            let data: Arc<[u8]> = value.into();

            if key == key2 {
                result = Some(data.clone());
            }

            write.entry(key2).or_insert(data);
        }

        if let Some(data) = result {
            Ok(data)
        } else {
            Err(AssetError::UnsupportedFormat(anyhow!(
                "The format of the asset file {key} is not compatible."
            )))
        }
    }

    /// Removes all the resources that are not used.
    pub fn clear(&self) {
        let mut map = self.map.write();

        // Clear all unused keys
        map.retain(|_key, value| Arc::strong_count(value) <= 1);
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            map: RwLock::new(HashMap::default()),
        }
    }
}

/// The cache holding each asset.
static CACHE: LazyLock<Cache> = LazyLock::new(Cache::default);
