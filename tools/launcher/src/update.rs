//! Instalar, atualizar, verificar e reparar o cliente pelo serviço `updates` (L3,
//! `docs/future/game-launcher.md` 4.6 e `docs/future/client-distribution-storage.md` 5.4).
//!
//! - O `versao.json` e cada manifesto só valem com a assinatura Ed25519 da chave fixada no launcher
//!   (`update-public-key.txt`, embutida no exe); o manifesto também tem de bater com o SHA-256 que o
//!   `versao.json` assinado declara.
//! - Cada arquivo baixado vai para `.vindictus-update/parts/<sha>.part` (retomável com `Range`), é
//!   descomprimido em fluxo, recebe a transformação de volta (`hfs-xor`) e é gravado em
//!   `<caminho>.novo` enquanto o SHA-256 é calculado; só um arquivo conferido fica preparado.
//! - Uma atualização da versão N-1 para N usa o delta do manifesto quando o arquivo local é o da versão
//!   anterior; um delta que não confere cai para o objeto inteiro. Versões puladas sobem uma a uma.
//! - Aplicar renomeia os `.novo` sobre os arquivos (o diário `aplicar.json` permite terminar uma
//!   aplicação interrompida) e só depois grava o `estado.json`. Os arquivos do jogador (`preserve`) só
//!   são instalados se faltarem.
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use vindictus_client_dist::manifest::{FileEntry, Manifest, VersionPointer};
use vindictus_client_dist::{codec, layout, paths, sign};

/// Chave pública do servidor de atualização (hex), gerada por `client-dist keygen`.
pub const PUBLIC_KEY: &str = include_str!("../update-public-key.txt");
/// Pasta de trabalho dentro da pasta do jogo.
pub const WORK: &str = ".vindictus-update";
const STATE: &str = "estado.json";
const JOURNAL: &str = "aplicar.json";
const MANIFEST_COPY: &str = "manifest.json";
const STAGED: &str = ".novo";
const MAX_DOCUMENT: u64 = 16 << 20;
const BUFFER: usize = 1 << 18;

#[derive(Debug)]
pub enum UpdateError {
    Offline(String),
    Server(u16),
    Signature,
    Invalid(&'static str),
    Digest(String),
    Io(String),
    Cancelled,
    GameRunning,
}

impl UpdateError {
    pub fn message(&self) -> String {
        match self {
            UpdateError::Offline(detail) => {
                format!("O servidor de atualização não respondeu ({detail}).")
            }
            UpdateError::Server(code) => {
                format!("O servidor de atualização recusou o pedido (HTTP {code}).")
            }
            UpdateError::Signature => {
                "A assinatura da atualização não confere: nada foi instalado.".into()
            }
            UpdateError::Invalid(what) => {
                format!("Resposta inválida do servidor de atualização ({what}).")
            }
            UpdateError::Digest(path) => {
                format!("O arquivo {path} veio corrompido; tente de novo.")
            }
            UpdateError::Io(text) => format!("Erro de disco: {text}"),
            UpdateError::Cancelled => "Cancelado.".into(),
            UpdateError::GameRunning => "Feche o jogo antes de atualizar.".into(),
        }
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        UpdateError::Io(error.to_string())
    }
}

impl From<vindictus_client_dist::DistError> for UpdateError {
    fn from(error: vindictus_client_dist::DistError) -> Self {
        match error {
            vindictus_client_dist::DistError::Signature(_)
            | vindictus_client_dist::DistError::Key => UpdateError::Signature,
            vindictus_client_dist::DistError::Digest(sha) => UpdateError::Digest(sha),
            vindictus_client_dist::DistError::Io(e) => UpdateError::Io(e.to_string()),
            vindictus_client_dist::DistError::Path(_) => UpdateError::Invalid("caminho"),
            vindictus_client_dist::DistError::Format(what) => UpdateError::Invalid(what),
        }
    }
}

/// Onde os documentos e objetos são lidos (HTTP no launcher, uma pasta nos testes).
pub trait Fetch: Sync {
    /// Um documento pequeno inteiro.
    fn bytes(&self, path: &str) -> Result<Vec<u8>, UpdateError>;
    /// Baixa `path` para `dest`, continuando um arquivo parcial; `progress` recebe os bytes novos.
    fn download(
        &self,
        path: &str,
        dest: &Path,
        size: u64,
        cancel: &AtomicBool,
        progress: &dyn Fn(u64),
    ) -> Result<(), UpdateError>;
}

/// O serviço `updates` por HTTP(S) (ureq com rustls).
pub struct HttpFetch {
    base: String,
    agent: ureq::Agent,
}

impl HttpFetch {
    pub fn new(base: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout_read(std::time::Duration::from_secs(30))
            .user_agent(concat!("vindictus-launcher/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            base: base.trim_end_matches('/').to_owned(),
            agent,
        }
    }

    fn get(&self, path: &str, from: u64) -> Result<ureq::Response, UpdateError> {
        let mut request = self.agent.get(&format!("{}/{path}", self.base));
        if from > 0 {
            request = request.set("Range", &format!("bytes={from}-"));
        }
        request.call().map_err(|error| match error {
            ureq::Error::Status(code, _) => UpdateError::Server(code),
            ureq::Error::Transport(t) => UpdateError::Offline(t.to_string()),
        })
    }
}

impl Fetch for HttpFetch {
    fn bytes(&self, path: &str) -> Result<Vec<u8>, UpdateError> {
        let mut out = Vec::new();
        self.get(path, 0)?
            .into_reader()
            .take(MAX_DOCUMENT + 1)
            .read_to_end(&mut out)?;
        if out.len() as u64 > MAX_DOCUMENT {
            return Err(UpdateError::Invalid("documento grande demais"));
        }
        Ok(out)
    }

    fn download(
        &self,
        path: &str,
        dest: &Path,
        size: u64,
        cancel: &AtomicBool,
        progress: &dyn Fn(u64),
    ) -> Result<(), UpdateError> {
        let have = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        if have == size {
            return Ok(());
        }
        let from = if have < size { have } else { 0 };
        let response = self.get(path, from)?;
        let append = from > 0 && response.status() == 206;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(dest)?;
        let mut written = if append { from } else { 0 };
        let mut reader = response.into_reader().take(size - written);
        let mut buffer = vec![0u8; BUFFER];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(UpdateError::Cancelled);
            }
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            file.write_all(&buffer[..n])?;
            written += n as u64;
            progress(n as u64);
        }
        if written != size {
            return Err(UpdateError::Offline("download incompleto".into()));
        }
        Ok(())
    }
}

/// `estado.json`: a versão instalada e o SHA-256 do manifesto dela.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalState {
    pub version: u32,
    pub manifest_sha256: String,
}

/// O que o servidor publica agora (assinatura conferida).
pub struct Remote {
    pub pointer: VersionPointer,
}

/// A chave fixada no exe.
pub fn pinned_key() -> Result<ed25519_dalek::VerifyingKey, UpdateError> {
    Ok(sign::verifying_key(PUBLIC_KEY)?)
}

/// `versao.json` assinado.
pub fn remote(fetch: &dyn Fetch, key: &ed25519_dalek::VerifyingKey) -> Result<Remote, UpdateError> {
    let bytes = fetch.bytes(layout::POINTER)?;
    let signature = fetch.bytes(&layout::signature(layout::POINTER))?;
    sign::verify(key, &bytes, &signature, "versao.json")?;
    Ok(Remote {
        pointer: VersionPointer::parse(&bytes)?,
    })
}

/// As notícias assinadas (`news.json`); sem notícias publicadas, a lista vazia.
pub fn news(
    fetch: &dyn Fetch,
    key: &ed25519_dalek::VerifyingKey,
) -> Result<Vec<vindictus_client_dist::manifest::NewsItem>, UpdateError> {
    let bytes = match fetch.bytes(layout::NEWS) {
        Ok(bytes) => bytes,
        Err(UpdateError::Server(404)) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let signature = fetch.bytes(&layout::signature(layout::NEWS))?;
    sign::verify(key, &bytes, &signature, "news.json")?;
    Ok(vindictus_client_dist::manifest::NewsFeed::parse(&bytes)?.items)
}

/// Manifesto `version` assinado; com `expected`, também o SHA-256 que o `versao.json` declara.
pub fn manifest(
    fetch: &dyn Fetch,
    key: &ed25519_dalek::VerifyingKey,
    version: u32,
    expected: Option<&str>,
) -> Result<(Manifest, Vec<u8>), UpdateError> {
    let name = layout::manifest(version);
    let bytes = fetch.bytes(&name)?;
    sign::verify(
        key,
        &bytes,
        &fetch.bytes(&layout::signature(&name))?,
        "manifest",
    )?;
    if expected.is_some_and(|sha| vindictus_client_dist::sha256_hex(&bytes) != sha) {
        return Err(UpdateError::Signature);
    }
    let manifest = Manifest::parse(&bytes)?;
    if manifest.version != version {
        return Err(UpdateError::Invalid("versão do manifesto"));
    }
    Ok((manifest, bytes))
}

pub fn local_state(game: &Path) -> Option<LocalState> {
    fs::read(game.join(WORK).join(STATE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

fn target(game: &Path, path: &str) -> Result<PathBuf, UpdateError> {
    paths::check(path)?;
    Ok(game.join(path.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

fn staged(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(STAGED);
    PathBuf::from(name)
}

fn sha256_file(
    path: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64),
) -> Result<String, UpdateError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; BUFFER];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(UpdateError::Cancelled);
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        progress(n as u64);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Como o trabalho anda (para a barra da janela).
#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub phase: &'static str,
    pub done: u64,
    pub total: u64,
    pub file: String,
}

/// O que fazer: verificar tudo (instalar, "já tenho o cliente", reparar) ou só o que mudou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// SHA-256 de todo arquivo que existe com o tamanho certo; baixa o resto.
    Full,
    /// Da versão do `estado.json` à publicada: só os caminhos que mudaram, com deltas.
    Changes,
}

/// O resultado de uma execução.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub version: u32,
    pub checked: usize,
    pub downloaded: usize,
    pub downloaded_bytes: u64,
    pub deltas: usize,
    pub delta_fallbacks: usize,
    pub preserved_kept: usize,
    pub removed: usize,
}

/// Termina uma aplicação interrompida (os `.novo` do diário) e grava o estado dela.
pub fn finish_interrupted(game: &Path) -> Result<Option<u32>, UpdateError> {
    let journal = game.join(WORK).join(JOURNAL);
    let Ok(bytes) = fs::read(&journal) else {
        return Ok(None);
    };
    let entry: Journal =
        serde_json::from_slice(&bytes).map_err(|_| UpdateError::Invalid("diário"))?;
    apply(game, &entry)?;
    Ok(Some(entry.state.version))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Journal {
    state: LocalState,
    paths: Vec<String>,
    /// Arquivos da versão anterior que a nova não tem (só os que o launcher instalou).
    #[serde(default)]
    remove: Vec<String>,
}

fn apply(game: &Path, journal: &Journal) -> Result<(), UpdateError> {
    for path in &journal.paths {
        let final_path = target(game, path)?;
        let novo = staged(&final_path);
        if novo.exists() {
            fs::rename(&novo, &final_path)?;
        }
    }
    for path in &journal.remove {
        let final_path = target(game, path)?;
        for file in [staged(&final_path), final_path] {
            match fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
    }
    write_json(&game.join(WORK).join(STATE), &journal.state)?;
    fs::remove_file(game.join(WORK).join(JOURNAL))?;
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), UpdateError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| UpdateError::Invalid("json"))?;
    let partial = path.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, path)?;
    Ok(())
}

/// Grava `<caminho>.novo` a partir do objeto baixado, em fluxo, conferindo tamanho e SHA-256.
fn restore_object(
    part: &Path,
    entry: &FileEntry,
    keystream: &[u8; codec::KEYSTREAM_SIZE],
    out: &Path,
    cancel: &AtomicBool,
) -> Result<(), UpdateError> {
    let mut decoder = zstd::stream::Decoder::new(fs::File::open(part)?)?;
    decoder.window_log_max(codec::MAX_WINDOW_LOG)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::with_capacity(BUFFER, fs::File::create(out)?);
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    let mut buffer = vec![0u8; BUFFER];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(UpdateError::Cancelled);
        }
        let n = decoder.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        let chunk = &mut buffer[..n];
        codec::apply(entry.transform, keystream, offset, chunk);
        hasher.update(&*chunk);
        writer.write_all(chunk)?;
        offset += n as u64;
        if offset > entry.size {
            break;
        }
    }
    writer.flush()?;
    drop(writer);
    if offset != entry.size || hex::encode(hasher.finalize()) != entry.sha256 {
        let _ = fs::remove_file(out);
        return Err(UpdateError::Digest(entry.path.clone()));
    }
    Ok(())
}

struct Run<'a> {
    game: &'a Path,
    fetch: &'a dyn Fetch,
    cancel: &'a AtomicBool,
    report: &'a dyn Fn(Progress),
    progress: std::cell::Cell<Progress>,
}

impl Run<'_> {
    fn step(&self, phase: &'static str, total: u64) {
        self.progress.set(Progress {
            phase,
            done: 0,
            total,
            file: String::new(),
        });
        (self.report)(self.progress.take_clone());
    }

    fn advance(&self, bytes: u64, file: &str) {
        let mut p = self.progress.take_clone();
        p.done += bytes;
        if p.file != file {
            p.file = file.to_owned();
        }
        self.progress.set(p.clone());
        (self.report)(p);
    }

    /// Baixa e prepara o objeto de `entry` (o `.part` sai depois de conferido).
    fn object(
        &self,
        entry: &FileEntry,
        keystream: &[u8; codec::KEYSTREAM_SIZE],
    ) -> Result<(), UpdateError> {
        let parts = self.game.join(WORK).join("parts");
        fs::create_dir_all(&parts)?;
        let part = parts.join(format!("{}.part", entry.sha256));
        let advance = |n: u64| self.advance(n, &entry.path);
        self.fetch.download(
            &layout::object(&entry.sha256),
            &part,
            entry.object_size,
            self.cancel,
            &advance,
        )?;
        let out = staged(&target(self.game, &entry.path)?);
        let result = restore_object(&part, entry, keystream, &out, self.cancel);
        if matches!(result, Err(UpdateError::Digest(_))) {
            // Um parcial corrompido não pode travar a próxima tentativa.
            let _ = fs::remove_file(&part);
        }
        result?;
        fs::remove_file(&part)?;
        Ok(())
    }

    /// Prepara pelo delta; `false` quando ele não serve (o chamador baixa o objeto).
    fn delta(
        &self,
        entry: &FileEntry,
        delta: &vindictus_client_dist::manifest::Delta,
        keystream: &[u8; codec::KEYSTREAM_SIZE],
    ) -> Result<bool, UpdateError> {
        let current = target(self.game, &entry.path)?;
        let Ok(mut old) = fs::read(&current) else {
            return Ok(false);
        };
        if old.len() as u64 != delta.from_size
            || vindictus_client_dist::sha256_hex(&old) != delta.from_sha256
        {
            return Ok(false);
        }
        let Ok(patch) = self
            .fetch
            .bytes(&layout::delta(&delta.from_sha256, &delta.to_sha256))
        else {
            return Ok(false);
        };
        self.advance(patch.len() as u64, &entry.path);
        codec::apply(entry.transform, keystream, 0, &mut old);
        let Ok(new) = codec::restore_from_delta(
            &old,
            &patch,
            entry.transform,
            keystream,
            &entry.sha256,
            entry.size,
        ) else {
            return Ok(false);
        };
        fs::write(staged(&current), new)?;
        Ok(true)
    }
}

trait TakeClone {
    fn take_clone(&self) -> Progress;
}

impl TakeClone for std::cell::Cell<Progress> {
    fn take_clone(&self) -> Progress {
        let value = self.take();
        self.set(value.clone());
        value
    }
}

fn keystream_of(
    fetch: &dyn Fetch,
    manifest: &Manifest,
) -> Result<[u8; codec::KEYSTREAM_SIZE], UpdateError> {
    let object = fetch.bytes(&layout::object(&manifest.keystream.sha256))?;
    let bytes = codec::decompress(&object, manifest.keystream.size)?;
    if vindictus_client_dist::sha256_hex(&bytes) != manifest.keystream.sha256 {
        return Err(UpdateError::Digest("hfs-keystream".into()));
    }
    Ok(codec::keystream(&bytes)?)
}

/// Leva a pasta do jogo à versão publicada. `game_running` é consultado antes de aplicar.
#[expect(
    clippy::too_many_arguments,
    reason = "one entry point for the window and the headless mode"
)]
pub fn run(
    game: &Path,
    fetch: &dyn Fetch,
    key: &ed25519_dalek::VerifyingKey,
    mode: Mode,
    version: Option<u32>,
    cancel: &AtomicBool,
    game_running: &dyn Fn() -> bool,
    report: &dyn Fn(Progress),
) -> Result<Outcome, UpdateError> {
    fs::create_dir_all(game.join(WORK))?;
    finish_interrupted(game)?;
    let remote = remote(fetch, key)?;
    let goal = version.unwrap_or(remote.pointer.current);
    if goal == 0 || goal > remote.pointer.current {
        return Err(UpdateError::Invalid("versão pedida"));
    }
    let local = local_state(game);
    let run = Run {
        game,
        fetch,
        cancel,
        report,
        progress: std::cell::Cell::new(Progress::default()),
    };
    let mut outcome = Outcome {
        version: goal,
        ..Outcome::default()
    };
    match (mode, local) {
        (Mode::Changes, Some(state)) if state.version < goal => {
            let (mut before, before_bytes) = manifest(fetch, key, state.version, None)?;
            if vindictus_client_dist::sha256_hex(&before_bytes) != state.manifest_sha256 {
                // O servidor republicou a versão local: só uma verificação completa sabe o que mudou.
                return Err(UpdateError::Invalid(
                    "versão local republicada; use Verificar arquivos",
                ));
            }
            for version in state.version + 1..=goal {
                let expected = (version == remote.pointer.current)
                    .then_some(remote.pointer.manifest_sha256.as_str());
                let (next, bytes) = manifest(fetch, key, version, expected)?;
                step(&run, &before, &next, &bytes, &mut outcome, game_running)?;
                before = next;
            }
        }
        (Mode::Changes, Some(state)) if state.version >= goal => outcome.version = state.version,
        _ => {
            let expected =
                (goal == remote.pointer.current).then_some(remote.pointer.manifest_sha256.as_str());
            let (next, bytes) = manifest(fetch, key, goal, expected)?;
            full(&run, &next, &bytes, &mut outcome, game_running)?;
        }
    }
    Ok(outcome)
}

/// Verifica todos os arquivos e baixa os que faltam ou diferem.
/// Caminhos da versão anterior que a nova não lista (sem diferença de maiúsculas). Os do jogador
/// (`preserve`) ficam: são dele.
fn removed_paths(before: &Manifest, next: &Manifest) -> Vec<String> {
    let kept: HashSet<String> = next
        .files
        .iter()
        .map(|f| f.path.to_ascii_lowercase())
        .collect();
    before
        .files
        .iter()
        .filter(|f| !f.preserve && !kept.contains(&f.path.to_ascii_lowercase()))
        .map(|f| f.path.clone())
        .collect()
}

/// O manifesto que o launcher aplicou por último (cópia em `.vindictus-update`), se bater com o estado.
fn applied_manifest(game: &Path) -> Option<Manifest> {
    let state = local_state(game)?;
    let bytes = fs::read(game.join(WORK).join(MANIFEST_COPY)).ok()?;
    (vindictus_client_dist::sha256_hex(&bytes) == state.manifest_sha256)
        .then(|| Manifest::parse(&bytes).ok())
        .flatten()
}

fn full(
    run: &Run<'_>,
    manifest: &Manifest,
    bytes: &[u8],
    outcome: &mut Outcome,
    game_running: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    let keystream = keystream_of(run.fetch, manifest)?;
    // Sem estado não se sabe o que o launcher instalou: nada é removido.
    let remove = applied_manifest(run.game)
        .map(|before| removed_paths(&before, manifest))
        .unwrap_or_default();
    let mut missing = Vec::new();
    let check_total: u64 = manifest
        .files
        .iter()
        .filter(|f| !f.preserve)
        .map(|f| f.size)
        .sum();
    run.step("Verificando arquivos", check_total);
    for entry in &manifest.files {
        let path = target(run.game, &entry.path)?;
        let size = fs::metadata(&path).map(|m| m.len()).ok();
        if entry.preserve {
            if size.is_none() {
                missing.push(entry);
            } else {
                outcome.preserved_kept += 1;
            }
            continue;
        }
        outcome.checked += 1;
        let advance = |n: u64| run.advance(n, &entry.path);
        let good =
            size == Some(entry.size) && sha256_file(&path, run.cancel, &advance)? == entry.sha256;
        if !good {
            if size != Some(entry.size) {
                run.advance(entry.size, &entry.path);
            }
            missing.push(entry);
        }
    }
    download(run, &missing, &keystream, outcome)?;
    outcome.removed += remove.len();
    commit(
        run.game,
        manifest,
        bytes,
        missing.iter().map(|e| e.path.clone()).collect(),
        remove,
        game_running,
    )
}

/// Uma versão a mais: só os caminhos que mudaram, com o delta quando o arquivo local é o anterior.
fn step(
    run: &Run<'_>,
    before: &Manifest,
    next: &Manifest,
    bytes: &[u8],
    outcome: &mut Outcome,
    game_running: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    let keystream = keystream_of(run.fetch, next)?;
    let old: HashMap<&str, &FileEntry> =
        before.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let deltas: HashMap<&str, &vindictus_client_dist::manifest::Delta> =
        next.deltas.iter().map(|d| (d.path.as_str(), d)).collect();
    let mut changed = Vec::new();
    for entry in &next.files {
        let path = target(run.game, &entry.path)?;
        let present = fs::metadata(&path).map(|m| m.len()).ok();
        let same = old
            .get(entry.path.as_str())
            .is_some_and(|o| o.sha256 == entry.sha256);
        if entry.preserve {
            if present.is_none() {
                changed.push(entry);
            }
        } else if !same || present != Some(entry.size) {
            changed.push(entry);
        }
    }
    let total = changed
        .iter()
        .map(|e| {
            deltas
                .get(e.path.as_str())
                .map_or(e.object_size, |d| d.delta_size)
        })
        .sum();
    run.step("Atualizando", total);
    let mut whole = Vec::new();
    for entry in &changed {
        match deltas.get(entry.path.as_str()) {
            Some(delta) if run.delta(entry, delta, &keystream)? => outcome.deltas += 1,
            Some(_) => {
                outcome.delta_fallbacks += 1;
                whole.push(*entry);
            }
            None => whole.push(*entry),
        }
    }
    download(run, &whole, &keystream, outcome)?;
    let remove = removed_paths(before, next);
    outcome.removed += remove.len();
    commit(
        run.game,
        next,
        bytes,
        changed.iter().map(|e| e.path.clone()).collect(),
        remove,
        game_running,
    )
}

fn download(
    run: &Run<'_>,
    entries: &[&FileEntry],
    keystream: &[u8; codec::KEYSTREAM_SIZE],
    outcome: &mut Outcome,
) -> Result<(), UpdateError> {
    let total = entries.iter().map(|e| e.object_size).sum();
    if !entries.is_empty() {
        run.step("Baixando", total);
    }
    let mut seen = HashSet::new();
    for entry in entries {
        run.object(entry, keystream)?;
        outcome.downloaded += 1;
        if seen.insert(entry.sha256.as_str()) {
            outcome.downloaded_bytes += entry.object_size;
        }
    }
    Ok(())
}

/// Com o jogo fechado: diário, renomeações, estado.
fn commit(
    game: &Path,
    manifest: &Manifest,
    bytes: &[u8],
    paths: Vec<String>,
    remove: Vec<String>,
    game_running: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    if game_running() {
        return Err(UpdateError::GameRunning);
    }
    let work = game.join(WORK);
    fs::write(work.join(MANIFEST_COPY), bytes)?;
    let journal = Journal {
        state: LocalState {
            version: manifest.version,
            manifest_sha256: vindictus_client_dist::sha256_hex(bytes),
        },
        paths,
        remove,
    };
    write_json(&work.join(JOURNAL), &journal)?;
    apply(game, &journal)?;
    let _ = fs::remove_dir(work.join("parts"));
    Ok(())
}

#[cfg(test)]
mod tests;
