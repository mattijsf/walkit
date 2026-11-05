// SPDX-License-Identifier: GPL-3.0-or-later

//! Skins read from `.wal` archives, with their siblings and the default element set.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use walkit::files::{self, LayeredFiles, MemoryFiles, SkinFiles, SkinsFolderFiles};

use walkit_trace::scenario::Scenario;

const MAX_ENTRY: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 8_192;

/// A `.wal`, which is a zip, read whole into memory. Sizes the archive claims can lie (a zip
/// bomb): what actually comes out is counted. Some archives hold the skin in one folder: it is
/// stripped when skin.xml sits there.
pub fn read_wal(path: &Path) -> Result<MemoryFiles, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut all = MemoryFiles::new();
    let mut names: Vec<String> = Vec::new();
    let mut total: u64 = 0;
    for i in 0..zip.len().min(MAX_ENTRIES) {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        let read = entry
            .by_ref()
            .take(MAX_ENTRY + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if read as u64 > MAX_ENTRY {
            continue;
        }
        total += read as u64;
        if total > MAX_TOTAL {
            break;
        }
        all.insert(&name, bytes);
        names.push(name);
    }
    let prefix = files::skin_folder(names.iter().map(|s| s.as_str())).unwrap_or_default();
    Ok(all.in_folder(&prefix))
}

/// The other `.wal` files of a Skins folder as sibling skins, by folder name ("BigBento.wal" is
/// "bigbento"), each read when a path first names it.
pub struct Siblings {
    by_name: HashMap<String, PathBuf>,
    loaded: RefCell<HashMap<String, Option<Rc<dyn SkinFiles>>>>,
}

impl Siblings {
    /// The `.wal` files in `dir`.
    pub fn new(dir: &Path) -> Rc<Siblings> {
        let mut by_name = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let path = e.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if let Some(stem) = name.strip_suffix(".wal") {
                    by_name.insert(stem.replace(' ', "").to_lowercase(), path.clone());
                }
            }
        }
        Rc::new(Siblings {
            by_name,
            loaded: RefCell::new(HashMap::new()),
        })
    }

    /// The sibling named `name`, read once.
    pub fn get(&self, name: &str) -> Option<Rc<dyn SkinFiles>> {
        if let Some(l) = self.loaded.borrow().get(name) {
            return l.clone();
        }
        let path = self.by_name.get(name)?;
        let files: Option<Rc<dyn SkinFiles>> = match read_wal(path) {
            Ok(f) => Some(Rc::new(f)),
            Err(e) => {
                eprintln!("sibling {}: {e}", path.display());
                None
            }
        };
        self.loaded
            .borrow_mut()
            .insert(name.to_string(), files.clone());
        files
    }
}

/// The files of a scenario's skin: the archive, its siblings and the default set layered as the
/// scenario asks.
pub fn skin_files(
    wal: &Path,
    siblings: &Rc<Siblings>,
    defaults: Option<&Path>,
    sc: &Scenario,
) -> Result<Box<dyn SkinFiles>, String> {
    let mut skin: Box<dyn SkinFiles> = Box::new(read_wal(wal)?);
    if !sc.no_siblings {
        let s = siblings.clone();
        skin = Box::new(SkinsFolderFiles::new(
            skin,
            Box::new(move |name| s.get(name)),
        ));
    }
    if let Some(defaults) = defaults.filter(|_| !sc.no_defaults) {
        if defaults.exists() {
            skin = Box::new(LayeredFiles::new(skin, Box::new(read_wal(defaults)?)));
        }
    }
    Ok(skin)
}
