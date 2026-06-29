use truck_modeling::{CompressedSolid, Solid};

use crate::cad_data::CadData;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

pub trait CadSerializer {
    fn file_type(&self) -> &'static str;
    fn extension(&self) -> &'static str;
    fn serialize<W: Write>(&self, data: &CadData, writer: &mut W) -> io::Result<()>;
    fn deserialize<R: Read>(&self, reader: &mut R) -> io::Result<CadData>;
}

pub struct IdaphsPrtFormat;

impl CadSerializer for IdaphsPrtFormat {
    fn file_type(&self) -> &'static str {
        "IDAPHS Part File"
    }

    fn extension(&self) -> &'static str {
        "idaphsprt"
    }

    fn serialize<W: Write>(&self, data: &CadData, writer: &mut W) -> io::Result<()> {
        // 例: マジックナンバーの書き込み
        writer.write_all(b"IDAPHS01")?;
        let compressed_solid = data.topology.compress();

        let json_string = serde_json::to_string(&compressed_solid).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("Serialization error: {}", e))
        })?;

        writer.write_all(json_string.as_bytes())?;

        Ok(())
    }

    fn deserialize<R: Read>(&self, reader: &mut R) -> io::Result<CadData> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != b"IDAPHS01" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic number",
            ));
        }

        // read the rest of the file into a string
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;

        let compressed_solid: CompressedSolid = serde_json::from_str(&buf).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Deserialization error: {}", e),
            )
        })?;

        let solid = Solid::extract(compressed_solid).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Solid extraction error: {}", e),
            )
        })?;

        Ok(CadData::from_solid(solid))
    }
}

pub struct CadFileManager;

impl CadFileManager {
    pub fn load_part_from_file<P: AsRef<Path>, S: CadSerializer>(
        path: P,
        serializer: S,
    ) -> io::Result<CadData> {
        let mut file = File::open(path)?;
        serializer.deserialize(&mut file)
    }

    pub fn save_part_to_file<P: AsRef<Path>, S: CadSerializer>(
        path: P,
        data: &CadData,
        serializer: S,
    ) -> io::Result<()> {
        let mut file = File::create(path)?;
        serializer.serialize(data, &mut file)
    }
}
