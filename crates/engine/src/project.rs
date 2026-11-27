use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, Image, LoadMode, Result, image::MAX_FILE};

const HEADER: &[u8; 8] = b"HEXPRJ\0\x01";
const MAX_METADATA: usize = 4 * 1024 * 1024;

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    name: String,
    mode: LoadMode,
    sha256: [u8; 32],
    bytes: usize,
    comments: BTreeMap<u64, String>,
    function_names: BTreeMap<u64, String>,
}

pub struct Project {
    image: Image,
    metadata: Metadata,
}

impl Project {
    pub fn new(name: String, image: Image) -> Self {
        let metadata = Metadata {
            name,
            mode: image.mode,
            sha256: Sha256::digest(image.bytes()).into(),
            bytes: image.bytes().len(),
            ..Metadata::default()
        };
        Self { image, metadata }
    }

    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn comment(&self, address: u64) -> Option<&str> {
        self.metadata.comments.get(&address).map(String::as_str)
    }

    pub fn function_name(&self, address: u64) -> Option<&str> {
        self.metadata
            .function_names
            .get(&address)
            .map(String::as_str)
    }

    pub fn set_comment(&mut self, address: u64, value: String) -> Result<()> {
        self.check_annotation(address, &value, 4_096)?;
        update_annotation(&mut self.metadata.comments, address, value);
        Ok(())
    }

    pub fn set_function_name(&mut self, address: u64, value: String) -> Result<()> {
        self.check_annotation(address, &value, 256)?;
        if value.chars().any(char::is_control) {
            return Err(Error::Input(
                "names cannot contain control characters".into(),
            ));
        }
        update_annotation(
            &mut self.metadata.function_names,
            address,
            value.trim().to_owned(),
        );
        Ok(())
    }

    fn check_annotation(&self, address: u64, value: &str, limit: usize) -> Result<()> {
        if self.image.file_offset(address).is_none() {
            return Err(Error::Input(
                "annotation address has no file mapping".into(),
            ));
        }
        if value.len() > limit {
            return Err(Error::Input(format!("annotation exceeds {limit} bytes")));
        }
        Ok(())
    }

    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = File::open(path)?;
        let mut header = [0; HEADER.len()];
        reader.read_exact(&mut header)?;
        if &header != HEADER {
            return Err(Error::Input("unrecognized Hexaris project format".into()));
        }
        let mut length = [0; 4];
        reader.read_exact(&mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        if length > MAX_METADATA {
            return Err(Error::Input("project metadata exceeds 4 MiB".into()));
        }
        let mut json = vec![0; length];
        reader.read_exact(&mut json)?;
        let metadata: Metadata = serde_json::from_slice(&json)?;
        if metadata.bytes == 0
            || metadata.bytes > MAX_FILE
            || reader.metadata()?.len() != (HEADER.len() + 4 + length + metadata.bytes) as u64
        {
            return Err(Error::Input("project has an invalid image size".into()));
        }
        let mut bytes = vec![0; metadata.bytes];
        reader.read_exact(&mut bytes)?;
        let checksum: [u8; 32] = Sha256::digest(&bytes).into();
        if checksum != metadata.sha256 {
            return Err(Error::Input("embedded binary checksum failed".into()));
        }
        let image = Image::from_bytes(bytes, metadata.mode)?;
        let project = Self { image, metadata };
        for (&address, value) in &project.metadata.comments {
            project.check_annotation(address, value, 4096)?;
        }
        for (&address, value) in &project.metadata.function_names {
            project.check_annotation(address, value, 256)?;
            if value.chars().any(char::is_control) {
                return Err(Error::Input(
                    "project contains an invalid function name".into(),
                ));
            }
        }
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.write(path)?
            .persist(path)
            .map_err(|error| error.error)?;
        Ok(())
    }

    pub fn save_new(&self, path: &Path) -> Result<()> {
        self.write(path)?
            .persist_noclobber(path)
            .map_err(|error| error.error)?;
        Ok(())
    }

    fn write(&self, path: &Path) -> Result<tempfile::NamedTempFile> {
        let json = serde_json::to_vec(&self.metadata)?;
        if json.len() > MAX_METADATA {
            return Err(Error::Input("project metadata exceeds 4 MiB".into()));
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(HEADER)?;
        file.write_all(&(json.len() as u32).to_le_bytes())?;
        file.write_all(&json)?;
        file.write_all(self.image.bytes())?;
        file.as_file().sync_all()?;
        Ok(file)
    }
}

fn update_annotation(annotations: &mut BTreeMap<u64, String>, address: u64, value: String) {
    if value.is_empty() {
        annotations.remove(&address);
    } else {
        annotations.insert(address, value);
    }
}
