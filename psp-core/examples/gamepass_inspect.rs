//! Dev-only troubleshooting harness for Xbox/gamepass (wgs) container dirs.
//! Parses `containers.index` leniently — reporting anomalies the app's strict
//! reader would only surface as a single error — then walks every file the
//! index references and reports whether the app could actually load each save.
//!
//! Usage: cargo run -p psp-core --example gamepass_inspect -- <container_dir>
//!            [--save <id>] [--all] [--deep] [--quiet-blobs]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use psp_core::gamepass::format::{guid_file_name, Filetime};

const INDEX_VERSION: u32 = 0xE;
const FILE_LIST_VERSION: u32 = 4;

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| "length overflow".to_string())?;
        if end > self.bytes.len() {
            return Err(format!(
                "want {count} bytes at offset {} but only {} remain",
                self.offset,
                self.bytes.len().saturating_sub(self.offset)
            ));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn uuid(&mut self) -> Result<uuid::Uuid, String> {
        Ok(uuid::Uuid::from_bytes(self.take(16)?.try_into().unwrap()))
    }

    fn utf16(&mut self) -> Result<String, String> {
        let start = self.offset;
        let units = self.u32()? as usize;
        if units == 0 {
            return Ok(String::new());
        }
        if units > 1 << 16 {
            return Err(format!("implausible string length {units} at offset {start}"));
        }
        decode_utf16(self.take(units * 2)?)
    }

    fn utf16_fixed(&mut self, units: usize) -> Result<String, String> {
        let value = decode_utf16(self.take(units * 2)?)?;
        Ok(value.trim_end_matches('\0').to_string())
    }
}

fn decode_utf16(bytes: &[u8]) -> Result<String, String> {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).map_err(|error| format!("invalid UTF-16: {error}"))
}

struct Entry {
    position: usize,
    offset: usize,
    name: String,
    name_repeat: String,
    cloud_id: String,
    seq: u8,
    flag: u32,
    uuid: uuid::Uuid,
    mtime: Filetime,
    reserved: u64,
    size: u64,
}

impl Entry {
    fn dir_name(&self) -> String {
        guid_file_name(&self.uuid)
    }

    fn save_id(&self) -> Option<&str> {
        self.name.split_once('-').map(|(id, _)| id)
    }

    /// The rejection the app's `ContainerEntry::read` raises; a single one of
    /// these aborts the whole index and every save with it.
    fn strict_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.name != self.name_repeat {
            errors.push(format!(
                "container name mismatch: {:?} != {:?}",
                self.name, self.name_repeat
            ));
        }
        let cloud_empty = self.cloud_id.is_empty();
        let cloud_flag = self.flag & 4 != 0;
        if (cloud_empty && !cloud_flag) || (!cloud_empty && cloud_flag) {
            errors.push(format!(
                "cloud id / flag mismatch: cloud_id={:?} flag={} (flag&4={})",
                self.cloud_id,
                self.flag,
                self.flag & 4
            ));
        }
        errors
    }
}

/// Mirrors `ContainerIndex::latest_save_containers` exactly, including its
/// substring matching — this is the classification the app actually applies.
fn container_key(name: &str) -> Option<String> {
    if name.contains("Players-") {
        let suffix = name.split("Players-").last().unwrap_or_default();
        Some(format!("Players-{suffix}"))
    } else if name.contains("LocalData") {
        Some("LocalData".to_string())
    } else if name.contains("LevelMeta") {
        Some("LevelMeta".to_string())
    } else if name.contains("Level") {
        Some("Level".to_string())
    } else if name.contains("WorldOption") {
        Some("WorldOption".to_string())
    } else {
        None
    }
}

fn wins_over(challenger: &Entry, holder: &Entry) -> bool {
    challenger.seq > holder.seq
        || (challenger.seq == holder.seq && challenger.mtime > holder.mtime)
}

fn format_filetime(value: Filetime) -> String {
    let seconds = value.to_unix_seconds();
    match chrono::DateTime::from_timestamp(seconds as i64, 0) {
        Some(stamp) => stamp.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => format!("raw {}", value.0),
    }
}

struct Index {
    version: u32,
    flag1: u32,
    package_name: String,
    mtime: Filetime,
    flag2: u32,
    index_uuid: String,
    unknown: u64,
    declared_count: u32,
    entries: Vec<Entry>,
    parse_error: Option<String>,
    trailing_bytes: usize,
}

fn parse_index(bytes: &[u8]) -> Result<Index, String> {
    let mut reader = Reader::new(bytes);
    let version = reader.u32()?;
    let declared_count = reader.u32()?;
    let flag1 = reader.u32()?;
    let package_name = reader.utf16()?;
    let mtime = Filetime(reader.u64()?);
    let flag2 = reader.u32()?;
    let index_uuid = reader.utf16()?;
    let unknown = reader.u64()?;

    let mut entries = Vec::new();
    let mut parse_error = None;
    for position in 0..declared_count as usize {
        let offset = reader.offset;
        match parse_entry(&mut reader, position, offset) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                parse_error = Some(format!("entry #{position} at offset {offset}: {error}"));
                break;
            }
        }
    }
    let trailing_bytes = bytes.len().saturating_sub(reader.offset);

    Ok(Index {
        version,
        flag1,
        package_name,
        mtime,
        flag2,
        index_uuid,
        unknown,
        declared_count,
        entries,
        parse_error,
        trailing_bytes,
    })
}

fn parse_entry(reader: &mut Reader, position: usize, offset: usize) -> Result<Entry, String> {
    let name = reader.utf16()?;
    let name_repeat = reader.utf16()?;
    let cloud_id = reader.utf16()?;
    let seq = reader.u8()?;
    let flag = reader.u32()?;
    let uuid = reader.uuid()?;
    let mtime = Filetime(reader.u64()?);
    let reserved = reader.u64()?;
    let size = reader.u64()?;
    Ok(Entry {
        position,
        offset,
        name,
        name_repeat,
        cloud_id,
        seq,
        flag,
        uuid,
        mtime,
        reserved,
        size,
    })
}

struct BlobFile {
    name: String,
    uuid: uuid::Uuid,
    blob_name: String,
    size: Option<u64>,
}

struct FileList {
    seq: u32,
    version: u32,
    declared_count: u32,
    files: Vec<BlobFile>,
    parse_error: Option<String>,
}

fn parse_file_list(list_path: &Path) -> Result<FileList, String> {
    let file_name = list_path.file_name().unwrap_or_default().to_string_lossy();
    let seq: u32 = file_name
        .strip_prefix("container.")
        .and_then(|suffix| suffix.parse().ok())
        .ok_or_else(|| format!("not a container.<seq> file: {file_name}"))?;
    let bytes = std::fs::read(list_path).map_err(|error| error.to_string())?;
    let parent = list_path.parent().unwrap_or_else(|| Path::new("."));

    let mut reader = Reader::new(&bytes);
    let version = reader.u32()?;
    let declared_count = reader.u32()?;
    let mut files = Vec::new();
    let mut parse_error = None;
    for position in 0..declared_count as usize {
        let record = (|| -> Result<BlobFile, String> {
            let name = reader.utf16_fixed(64)?;
            let _cloud_uuid = reader.uuid()?;
            let uuid = reader.uuid()?;
            let blob_name = guid_file_name(&uuid);
            let size = std::fs::metadata(parent.join(&blob_name))
                .ok()
                .map(|meta| meta.len());
            Ok(BlobFile {
                name,
                uuid,
                blob_name,
                size,
            })
        })();
        match record {
            Ok(file) => files.push(file),
            Err(error) => {
                parse_error = Some(format!("file record #{position}: {error}"));
                break;
            }
        }
    }
    Ok(FileList {
        seq,
        version,
        declared_count,
        files,
        parse_error,
    })
}

fn describe_payload(data: &[u8], deep: bool) -> String {
    if data.len() < 12 {
        return format!("{} bytes — too short to be a .sav", data.len());
    }
    let magic = &data[8..12];
    let label = if magic == b"PlM1" {
        "PlM1 (Oodle)"
    } else if &magic[..3] == b"PlZ" {
        "PlZ (zlib)"
    } else if &data[..3] == b"CNK" {
        "CNK (chunked)"
    } else if &data[..4] == b"GVAS" {
        "GVAS (uncompressed)"
    } else {
        "unrecognized magic"
    };
    let head = data[..12]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");

    let decompressed = psp_core::ue::compression::decompress_save(&mut std::io::Cursor::new(data));
    let decode = match &decompressed {
        Ok(gvas) => format!("decompress OK ({} bytes GVAS)", gvas.len()),
        Err(error) => format!("DECOMPRESS FAILED: {error}"),
    };
    let parse = if deep {
        match psp_core::savio::read_sav_bytes(data) {
            Ok(save) => format!(", GVAS parse OK ({} root props)", save.root.properties.0.len()),
            Err(error) => format!(", GVAS PARSE FAILED: {error}"),
        }
    } else {
        String::new()
    };
    format!("{label} [{head}] — {decode}{parse}")
}

struct Options {
    container_dir: PathBuf,
    save_filter: Option<String>,
    list_all: bool,
    deep: bool,
    quiet_blobs: bool,
}

fn parse_args() -> Options {
    let mut args = std::env::args().skip(1);
    let mut container_dir = None;
    let mut save_filter = None;
    let mut list_all = false;
    let mut deep = false;
    let mut quiet_blobs = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--save" => save_filter = args.next(),
            "--all" => list_all = true,
            "--deep" => deep = true,
            "--quiet-blobs" => quiet_blobs = true,
            other => container_dir = Some(PathBuf::from(other)),
        }
    }
    Options {
        container_dir: container_dir.unwrap_or_else(|| {
            eprintln!(
                "usage: gamepass_inspect <container_dir> [--save <id>] [--all] [--deep] [--quiet-blobs]"
            );
            std::process::exit(2);
        }),
        save_filter,
        list_all,
        deep,
        quiet_blobs,
    }
}

fn main() {
    let options = parse_args();
    let container_dir = &options.container_dir;
    let index_path = container_dir.join("containers.index");
    let bytes = match std::fs::read(&index_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("cannot read {}: {error}", index_path.display());
            std::process::exit(1);
        }
    };
    let index = match parse_index(&bytes) {
        Ok(index) => index,
        Err(error) => {
            eprintln!("containers.index header is unreadable: {error}");
            std::process::exit(1);
        }
    };

    let app = AppView::load(container_dir);
    print_header(&index, &index_path, bytes.len());
    print_integrity(&index);
    let dirs_on_disk = list_container_dirs(container_dir);
    print_saves(&index, &app, container_dir, &dirs_on_disk, &options);
    print_file_walk(&index, container_dir, &options);
    print_orphans(&index, &dirs_on_disk);
}

fn list_container_dirs(container_dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(container_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|dir_entry| dir_entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|dir_entry| dir_entry.file_name().to_string_lossy().to_string())
        .collect()
}

fn print_header(index: &Index, index_path: &Path, byte_len: usize) {
    println!("=== containers.index ===");
    println!("path            {}", index_path.display());
    println!("file size       {byte_len} bytes");
    println!(
        "version         {} ({})",
        index.version,
        if index.version == INDEX_VERSION {
            "supported".to_string()
        } else {
            format!("UNSUPPORTED — app requires {INDEX_VERSION}")
        }
    );
    println!(
        "container count {} declared, {} parsed",
        index.declared_count,
        index.entries.len()
    );
    println!("package         {:?}", index.package_name);
    println!(
        "index mtime     {} ({})",
        index.mtime.0,
        format_filetime(index.mtime)
    );
    println!("index uuid      {:?}", index.index_uuid);
    println!("flag1 / flag2   {} / {}", index.flag1, index.flag2);
    println!("unknown         {}", index.unknown);
    println!("trailing bytes  {}", index.trailing_bytes);
    if let Some(error) = &index.parse_error {
        println!("PARSE ABORTED   {error}");
    }
    println!();
}

fn print_integrity(index: &Index) {
    println!("=== integrity (what the app's strict reader would reject) ===");
    let mut problems = 0;
    if index.version != INDEX_VERSION {
        println!("  FATAL: unsupported index version {}", index.version);
        problems += 1;
    }
    if let Some(error) = &index.parse_error {
        println!("  FATAL: {error}");
        problems += 1;
    }
    if index.trailing_bytes != 0 {
        println!(
            "  WARN: {} unparsed bytes after the last entry",
            index.trailing_bytes
        );
        problems += 1;
    }
    for entry in &index.entries {
        for error in entry.strict_errors() {
            println!(
                "  FATAL: entry #{} ({:?}) at offset {}: {error}",
                entry.position, entry.name, entry.offset
            );
            problems += 1;
        }
    }
    if problems == 0 {
        println!("  none — the index parses cleanly under the app's reader");
    }
    println!();
}

/// What psp-core itself makes of the directory. The lenient parse above exists to
/// explain a directory the app rejects outright; wherever the app CAN read it, its
/// own answers are the authoritative ones and get reported instead of a replica.
struct AppView {
    index: Option<psp_core::gamepass::format::ContainerIndex>,
    listed: Result<Vec<(String, String)>, String>,
}

impl AppView {
    fn load(container_dir: &Path) -> Self {
        AppView {
            index: psp_core::gamepass::format::ContainerIndex::read_from_dir(container_dir).ok(),
            listed: psp_core::gamepass::scan::scan_saves(container_dir)
                .map(|saves| {
                    saves
                        .iter()
                        .map(|(id, data)| (id.clone(), data.world_name.clone()))
                        .collect()
                })
                .map_err(|error| error.to_string()),
        }
    }

    fn world_name(&self, save_id: &str) -> Option<&str> {
        self.listed.as_ref().ok().and_then(|saves| {
            saves
                .iter()
                .find(|(id, _)| id == save_id)
                .map(|(_, name)| name.as_str())
        })
    }
}

/// The app's own `latest_save_containers` verdict, keyed the way the report groups
/// candidates. Falls back to the lenient replica only when psp-core cannot read the
/// index at all — in which case nothing loads anyway and the replica is the only
/// way to show why.
fn resolve_selection(
    app: &AppView,
    save_id: &str,
    container_dir: &Path,
    candidates: &BTreeMap<String, Vec<&Entry>>,
) -> BTreeMap<String, (uuid::Uuid, String)> {
    if let Some(index) = &app.index {
        return index
            .latest_save_containers(save_id, container_dir)
            .iter()
            .map(|(key, entry)| {
                (
                    key.clone(),
                    (entry.container_uuid, entry.container_name.clone()),
                )
            })
            .collect();
    }
    let mut selection = BTreeMap::new();
    for (key, group) in candidates {
        let mut winner: Option<&Entry> = None;
        for entry in group {
            let replace = match winner {
                None => true,
                Some(current) => wins_over(entry, current),
            };
            if replace {
                winner = Some(entry);
            }
        }
        if let Some(winner) = winner {
            selection.insert(key.clone(), (winner.uuid, winner.name.clone()));
        }
    }
    selection
}

fn print_saves(
    index: &Index,
    app: &AppView,
    container_dir: &Path,
    dirs_on_disk: &BTreeSet<String>,
    options: &Options,
) {
    let mut by_save: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    let mut unscoped: Vec<&Entry> = Vec::new();
    for entry in &index.entries {
        match entry.save_id() {
            Some(save_id) => by_save
                .entry(save_id.to_string())
                .or_default()
                .push(entry),
            None => unscoped.push(entry),
        }
    }

    println!(
        "=== saves ({} save ids, {} non-save containers) ===",
        by_save.len(),
        unscoped.len()
    );
    println!();

    for (save_id, entries) in &by_save {
        if let Some(filter) = &options.save_filter {
            if save_id != filter {
                continue;
            }
        }
        let present = entries
            .iter()
            .filter(|entry| dirs_on_disk.contains(&entry.dir_name()))
            .count();
        println!("--- save {save_id} ---");
        println!(
            "  {} containers in index, {present} with a blob dir on disk",
            entries.len()
        );

        let mut candidates: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
        for entry in entries {
            if let Some(key) = container_key(&entry.name) {
                candidates.entry(key).or_default().push(entry);
            } else {
                println!(
                    "  UNCLASSIFIED  {:?} (no Level/LevelMeta/LocalData/WorldOption/Players- match — the app ignores it)",
                    entry.name
                );
            }
        }

        let selection = resolve_selection(app, save_id, container_dir, &candidates);

        for (key, group) in &candidates {
            let winner = selection.get(key);
            let is_winner = |entry: &Entry| {
                winner
                    .map(|(id, name)| entry.uuid == *id && entry.name == *name)
                    .unwrap_or(false)
            };
            let winner_present = winner
                .map(|(id, _)| dirs_on_disk.contains(&guid_file_name(id)))
                .unwrap_or(false);
            let rescuable = !winner_present
                && group
                    .iter()
                    .any(|entry| dirs_on_disk.contains(&entry.dir_name()));
            println!("  [{key}]");
            for entry in group {
                let on_disk = dirs_on_disk.contains(&entry.dir_name());
                let chosen = is_winner(entry);
                if !options.list_all && !chosen && !on_disk {
                    continue;
                }
                println!(
                    "    {} seq={:<4} mtime={} dir={} {} size={} name={:?}",
                    if chosen { "->" } else { "  " },
                    entry.seq,
                    format_filetime(entry.mtime),
                    entry.dir_name(),
                    if on_disk { "PRESENT" } else { "MISSING " },
                    entry.size,
                    entry.name
                );
            }
            if !options.list_all {
                let hidden = group
                    .iter()
                    .filter(|entry| !dirs_on_disk.contains(&entry.dir_name()) && !is_winner(entry))
                    .count();
                if hidden > 0 {
                    println!("       ({hidden} more cloud-only candidates hidden; pass --all)");
                }
            }
            if rescuable {
                println!(
                    "    ^^ SELECTION DEFECT: the winning container has no blob dir on disk, \
                     but another candidate for this key does."
                );
            }
        }

        print_scan_verdict(save_id, app, &selection, container_dir, dirs_on_disk, options);
        println!();
    }

    if !unscoped.is_empty() && options.save_filter.is_none() {
        println!("--- non-save containers ---");
        for entry in &unscoped {
            println!(
                "  seq={:<4} dir={} {} size={} name={:?}",
                entry.seq,
                entry.dir_name(),
                if dirs_on_disk.contains(&entry.dir_name()) {
                    "PRESENT"
                } else {
                    "MISSING "
                },
                entry.size,
                entry.name
            );
        }
        println!();
    }
}

/// Reports whether `gamepass::scan::scan_saves` actually listed this save, then
/// walks the containers the app selected for it and says what each one yields.
fn print_scan_verdict(
    save_id: &str,
    app: &AppView,
    selection: &BTreeMap<String, (uuid::Uuid, String)>,
    container_dir: &Path,
    dirs_on_disk: &BTreeSet<String>,
    options: &Options,
) {
    match (&app.listed, app.world_name(save_id)) {
        (Err(error), _) => {
            println!("  VERDICT: the app cannot read this container dir at all — {error}");
            return;
        }
        (Ok(_), Some(name)) => println!("  VERDICT: the browser LISTS this save as {name:?}"),
        (Ok(_), None) => {
            print!("  VERDICT: the browser SKIPS this save — ");
            match selection.get("LevelMeta") {
                None => println!("no LevelMeta container for this save id"),
                Some((id, _)) if !dirs_on_disk.contains(&guid_file_name(id)) => println!(
                    "the chosen LevelMeta blob dir {} does not exist",
                    guid_file_name(id)
                ),
                Some(_) => println!("its LevelMeta blob is unreadable or has no world name"),
            }
        }
    }

    for (key, (id, name)) in selection {
        let Some(entry) = find_entry(app, id, name) else {
            continue;
        };
        if !dirs_on_disk.contains(&guid_file_name(id)) {
            println!("           LOAD DEFECT: chosen [{key}] container has no blob dir on disk");
            continue;
        }
        match psp_core::gamepass::store::read_first_blob(container_dir, &entry) {
            Ok(Some((seq, data))) => {
                if !options.quiet_blobs {
                    println!(
                        "           [{key}] container.{seq} {}",
                        describe_payload(&data, options.deep)
                    );
                }
            }
            Ok(None) => println!("           LOAD DEFECT: [{key}] dir holds no readable blob"),
            Err(error) => println!("           LOAD DEFECT: [{key}] unreadable: {error}"),
        }
    }
}

fn find_entry(
    app: &AppView,
    id: &uuid::Uuid,
    name: &str,
) -> Option<psp_core::gamepass::format::ContainerEntry> {
    app.index
        .as_ref()?
        .containers
        .iter()
        .find(|entry| entry.container_uuid == *id && entry.container_name == name)
        .cloned()
}

fn print_file_walk(index: &Index, container_dir: &Path, options: &Options) {
    println!("=== file walk (every container.<seq> present on disk) ===");
    let mut walked = 0;
    for entry in &index.entries {
        if let Some(filter) = &options.save_filter {
            if entry.save_id() != Some(filter.as_str()) {
                continue;
            }
        }
        let blob_dir = container_dir.join(entry.dir_name());
        if !blob_dir.is_dir() {
            continue;
        }
        walked += 1;
        println!("--- {} ({:?}) ---", entry.dir_name(), entry.name);
        println!(
            "  index says: seq={} size={} mtime={} reserved={}",
            entry.seq,
            entry.size,
            format_filetime(entry.mtime),
            entry.reserved
        );

        let mut list_paths: Vec<PathBuf> = Vec::new();
        let mut stray: Vec<String> = Vec::new();
        for dir_entry in std::fs::read_dir(&blob_dir).into_iter().flatten().flatten() {
            let name = dir_entry.file_name().to_string_lossy().to_string();
            if name.starts_with("container.") {
                list_paths.push(dir_entry.path());
            } else {
                stray.push(name);
            }
        }
        list_paths.sort();
        if list_paths.is_empty() {
            println!("  NO container.<seq> file list in this dir");
        }

        let mut referenced: BTreeSet<String> = BTreeSet::new();
        for list_path in &list_paths {
            match parse_file_list(list_path) {
                Ok(file_list) => {
                    println!(
                        "  container.{}: version={} ({}), {} files declared, {} parsed",
                        file_list.seq,
                        file_list.version,
                        if file_list.version == FILE_LIST_VERSION {
                            "supported"
                        } else {
                            "UNSUPPORTED"
                        },
                        file_list.declared_count,
                        file_list.files.len()
                    );
                    if let Some(error) = &file_list.parse_error {
                        println!("    PARSE ABORTED: {error}");
                    }
                    for file in &file_list.files {
                        referenced.insert(file.blob_name.clone());
                        match file.size {
                            Some(size) => {
                                println!(
                                    "    {:?} uuid={} blob={} {} bytes",
                                    file.name, file.uuid, file.blob_name, size
                                );
                                if !options.quiet_blobs {
                                    match std::fs::read(blob_dir.join(&file.blob_name)) {
                                        Ok(data) => println!(
                                            "        {}",
                                            describe_payload(&data, options.deep)
                                        ),
                                        Err(error) => {
                                            println!("        BLOB UNREADABLE: {error}")
                                        }
                                    }
                                }
                            }
                            None => println!(
                                "    {:?} uuid={} blob={} MISSING — the app silently skips this file",
                                file.name, file.uuid, file.blob_name
                            ),
                        }
                    }
                }
                Err(error) => println!(
                    "  {}: UNPARSEABLE — {error}",
                    list_path.file_name().unwrap_or_default().to_string_lossy()
                ),
            }
        }
        for name in stray {
            if !referenced.contains(&name) {
                println!("  ORPHAN BLOB: {name} (present on disk, referenced by no file list)");
            }
        }
        println!();
    }
    if walked == 0 {
        println!("  no referenced container dir exists on disk");
        println!();
    }
}

fn print_orphans(index: &Index, dirs_on_disk: &BTreeSet<String>) {
    let referenced: BTreeSet<String> = index
        .entries
        .iter()
        .map(|entry| entry.dir_name())
        .collect();
    let orphans: Vec<&String> = dirs_on_disk.difference(&referenced).collect();
    println!("=== orphan dirs (on disk, no index entry) ===");
    if orphans.is_empty() {
        println!("  none");
    }
    for name in orphans {
        println!("  {name}");
    }
    println!();
}
