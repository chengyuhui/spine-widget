use std::{fs::File, io::Read};

use anyhow::Result;

pub fn load_file_packed(path: &str) -> Result<Vec<u8>> {
    let mut buf = vec![];

    if let Some((zip_path, file_path)) = path.split_once("??/") {
        let zip_file = File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(zip_file)?;
        let mut file = archive.by_name(file_path)?;
        file.read_to_end(&mut buf)?;
    } else {
        std::fs::File::open(path)?.read_to_end(&mut buf)?;
    }
    
    Ok(buf)
}
