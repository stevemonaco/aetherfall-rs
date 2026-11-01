//! Release builds ship the licensed files under `assets/external` scrambled (see `tools/pack-assets.ps1`) so they are
//! never distributed as usable standalone files. This asset reader unscrambles them as they load.

use std::path::Path;

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceId, ErasedAssetReader, PathStream,
    Reader, VecReader,
};
use bevy::prelude::*;

/// Must match the seed in `tools/pack-assets.ps1`.
const SEED: u32 = 0x5EED_AE7F;

/// Replaces the default asset source; must be called before `AssetPlugin` is added.
pub fn register(app: &mut App) {
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(|| Box::new(PackedReader(AssetSource::get_default_reader("assets".into())()))),
    );
}

/// XORs the bytes with an LCG keystream; applying it twice restores the original.
fn unscramble(bytes: &mut [u8]) {
    let mut state = SEED;
    for byte in bytes {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *byte ^= (state >> 24) as u8;
    }
}

struct PackedReader(Box<dyn ErasedAssetReader>);

impl AssetReader for PackedReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let mut bytes = Vec::new();
        self.0.read(path).await?.read_to_end(&mut bytes).await?;
        if path.starts_with("external") {
            unscramble(&mut bytes);
        }
        Ok(VecReader::new(bytes))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.0.read_meta(path).await
    }

    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> {
        self.0.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.0.is_directory(path).await
    }
}
