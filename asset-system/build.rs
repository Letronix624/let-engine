use ahash::HashMap;
use anyhow::{Error, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use toml::{map::Map, Table};

fn main() -> Result<()> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let map_data_path = out_dir.join("map_data");
    fs::write(&map_data_path, [0, 0])?;
    let Ok(assets_path) = std::env::var("ASSETS_DIR") else {
        return Ok(());
    };

    println!("cargo:rerun-if-changed={}", assets_path);
    let assets_path = PathBuf::from(assets_path);
    let output_path = PathBuf::from(&out_dir).join("../../..");
    let config = {
        let contents = fs::read_to_string(assets_path.join("config.toml"))
            .map_err(|_| Error::msg("Could not find config.toml in the set assets folder."))?;
        toml::from_str::<Config>(&contents)?
    };

    let max_size = config.max_size.unwrap_or_default();
    let naming = config.naming.unwrap_or_default();
    let output = config.output.unwrap_or_default();

    let target = output_path.join(&output);
    fs::create_dir_all(&target)?;

    let mut exclude = config.exclude.unwrap_or(vec![]);
    for path in exclude.iter_mut() {
        if path.is_relative() {
            *path = assets_path.join(path.clone());
        }
    }

    let mut map: HashMap<String, (PathBuf, Compression)> = HashMap::default();

    for group in config.groups.iter() {
        let name = group.0;
        let group_path = assets_path.join(group.1.as_str().unwrap());
        if !group_path.exists() {
            return Err(Error::msg(format!("Group {name} does not exist.")));
        }
        if group_path.is_file() {
            return Err(Error::msg(format!("Group {name} is not a directory.")));
        }

        let group_config = {
            let contents = fs::read_to_string(group_path.join("config.toml")).ok();
            if let Some(contents) = contents {
                println!("Group config for {group_path:?} found.");
                Some(toml::from_str::<GroupConfig>(&contents)?)
            } else {
                println!("Group config for {group_path:?} not found.");
                None
            }
        };
        println!("Content is {:?}.", group_config);
        let compression: Compression = {
            if let Some(config) = group_config.clone() {
                config
                    .compression
                    .map(Compression::from_string)
                    .unwrap_or_default()
            } else {
                Compression::None
            }
        };
        let compression_level = {
            if let Some(group_config) = &group_config {
                group_config.compression_level.unwrap_or_default()
            } else {
                3
            }
        };
        let compression_level = clamp_to_compression_level(compression_level, &compression);
        let naming = if let Some(config) = group_config.clone() {
            config.naming.unwrap_or(naming.clone())
        } else {
            naming.clone()
        };
        let max_size = {
            if let Some(config) = group_config {
                config.max_size.unwrap_or(max_size)
            } else {
                max_size
            }
        };

        let sections = sort_groups(path_tree(&group_path)?, max_size);
        for (id, section) in sections.into_iter().enumerate() {
            let mut file = fs::File::create(
                target.join(naming.replace("%g", name).replace("%i", &id.to_string())),
            )?;
            let binary_relative =
                output.join(naming.replace("%g", name).replace("%i", &id.to_string()));
            let mut assets: File = HashMap::default();
            for path in section {
                let relative_path = path
                    .strip_prefix(&assets_path)?
                    .to_string_lossy()
                    .to_string();

                map.insert(
                    relative_path.clone(),
                    (binary_relative.clone(), compression),
                );
                if exclude.contains(&path) {
                    continue;
                }
                let data = fs::read(&path)?;
                assets.insert(relative_path, data);
            }
            // Serialize the HashMap into Bincode
            let mut assets = bincode::serialize(&assets)?;
            // Compress the data if enabled
            let compressed_data = compression.compress(&assets, compression_level)?;
            assets = compressed_data;
            // Write named group split asset file into the output dir.
            file.write_all(&assets)?;
        }
    }

    let map_data = bincode::serialize(&map)?;
    fs::write(map_data_path, map_data)?;

    Ok(())
}

type File = HashMap<String, Vec<u8>>;

fn sort_groups(groups: Vec<(PathBuf, usize)>, threshold: usize) -> Vec<Vec<PathBuf>> {
    let mut result: Vec<Vec<PathBuf>> = Vec::new();
    let mut current_vec: Vec<PathBuf> = Vec::new();
    let mut current_sum = 0;

    for group in groups {
        if group.1 > threshold {
            result.push(vec![group.0]);
        } else {
            if current_sum + group.1 > threshold && !current_vec.is_empty() {
                result.push(current_vec.clone());
                current_vec.clear();
                current_sum = 0;
            }
            current_sum += group.1;
            current_vec.push(group.0);
        }
    }

    if !current_vec.is_empty() {
        result.push(current_vec);
    }

    result
}

/// Configuration file for assets.
#[derive(Deserialize, Clone, Debug)]
struct Config {
    pub max_size: Option<usize>,
    pub naming: Option<String>,
    pub exclude: Option<Vec<PathBuf>>,
    pub output: Option<PathBuf>,
    pub groups: Table,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_size: Some(30_000_000),
            naming: Some(String::from("%g%i")),
            exclude: None,
            output: Some(".".into()),
            groups: Map::new(),
        }
    }
}

/// Configuration file for assets.
#[derive(Deserialize, Clone, Debug)]
struct GroupConfig {
    pub max_size: Option<usize>,
    pub compression: Option<String>,
    pub compression_level: Option<u32>,
    pub naming: Option<String>,
}

impl Default for GroupConfig {
    fn default() -> Self {
        Self {
            compression: None,
            compression_level: Some(5),
            naming: Some(String::from("%g%i")),
            max_size: Some(30_000_000),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Compression {
    #[default]
    None,
    #[cfg(feature = "deflate")]
    Deflate,
    #[cfg(feature = "bzip2")]
    Bwt,
    #[cfg(feature = "zstd")]
    Zstd,
    #[cfg(feature = "lzma")]
    Lzma,
    #[cfg(feature = "lz4")]
    Lz4,
}

impl Compression {
    #[allow(unused_variables)]
    pub fn compress(&self, buffer: &[u8], compression_level: u32) -> Result<Vec<u8>> {
        #[allow(unused_assignments)]
        let mut compressed = vec![];
        match self {
            Compression::None => {
                compressed = buffer.to_vec();
            }
            #[cfg(feature = "deflate")]
            Compression::Deflate => {
                let mut encoder = flate2::write::GzEncoder::new(
                    &mut compressed,
                    flate2::Compression::new(compression_level),
                );
                encoder.write_all(buffer)?;
                encoder.finish()?.flush()?;
            }
            #[cfg(feature = "bzip2")]
            Compression::Bwt => {
                let mut encoder = bzip2::write::BzEncoder::new(
                    &mut compressed,
                    bzip2::Compression::new(compression_level),
                );
                encoder.write_all(buffer)?;
                encoder.finish()?.flush()?;
            }
            #[cfg(feature = "zstd")]
            Compression::Zstd => {
                let mut encoder = zstd::Encoder::new(&mut compressed, compression_level as i32)?;
                encoder.write_all(buffer)?;
                encoder.finish()?.flush()?;
            }
            #[cfg(feature = "lzma")]
            Compression::Lzma => {
                let mut encoder = xz2::write::XzEncoder::new(&mut compressed, compression_level);
                encoder.write_all(buffer)?;
                encoder.finish()?.flush()?;
            }
            #[cfg(feature = "lz4")]
            Compression::Lz4 => {
                let mut encoder = lz4::EncoderBuilder::new()
                    .favor_dec_speed(true)
                    .level(compression_level)
                    .build(&mut compressed)?;
                encoder.write_all(buffer)?;
                encoder.flush()?;
                let result = encoder.finish();
                result.0.flush()?;
                result.1?;
            }
        }
        Ok(compressed)
    }

    fn from_string(string: String) -> Self {
        match string.to_lowercase().as_str() {
            #[cfg(feature = "deflate")]
            "deflate" => Compression::Deflate,
            #[cfg(feature = "deflate")]
            "flate" => Compression::Deflate,
            #[cfg(feature = "bzip2")]
            "bwt" => Compression::Bwt,
            #[cfg(feature = "bzip2")]
            "burrows-wheeler-transform" => Compression::Bwt,
            #[cfg(feature = "bzip2")]
            "bzip2" => Compression::Bwt,
            #[cfg(feature = "bzip2")]
            "bzip" => Compression::Bwt,
            #[cfg(feature = "bzip2")]
            "bz" => Compression::Bwt,
            #[cfg(feature = "zstd")]
            "zstd" => Compression::Zstd,
            #[cfg(feature = "zstd")]
            "zstandard" => Compression::Zstd,
            #[cfg(feature = "lzma")]
            "lzma" => Compression::Lzma,
            #[cfg(feature = "lzma")]
            "lempel-ziv-markov-chain-algorithm" => Compression::Lzma,
            #[cfg(feature = "lz4")]
            "lz4" => Compression::Lz4,
            _ => panic!("the given compression in the config does not exist."),
        }
    }
}

fn clamp_to_compression_level(input: u32, compression: &Compression) -> u32 {
    #[allow(unused)]
    match compression {
        Compression::None => input,
        #[cfg(feature = "deflate")]
        Compression::Deflate => input.clamp(0, 9),
        #[cfg(feature = "bzip2")]
        Compression::Bwt => input.clamp(0, 9),
        #[cfg(feature = "zstd")]
        Compression::Zstd => input.clamp(0, 22),
        #[cfg(feature = "lzma")]
        Compression::Lzma => input.clamp(0, 9),
        #[cfg(feature = "lz4")]
        Compression::Lz4 => input.clamp(0, 16),
    }
}

/// Replicates a system path in this structure.
pub fn path_tree<P: AsRef<Path>>(path: P) -> Result<Vec<(PathBuf, usize)>> {
    let mut paths = Vec::new();
    generate_path_tree(path, &mut paths)?;

    Ok(paths)
}
pub fn generate_path_tree<P: AsRef<Path>>(
    path: P,
    paths: &mut Vec<(PathBuf, usize)>,
) -> Result<()> {
    let path = path.as_ref();

    if path.is_file() {
        let data = fs::read(path)?;
        let size = data.len();
        paths.push((path.to_path_buf(), size));
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        generate_path_tree(path, paths)?;
    }

    Ok(())
}
