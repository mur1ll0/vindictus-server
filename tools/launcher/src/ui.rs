//! Janela do launcher em Win32 puro (user32/gdi32/comctl32/shell32/ole32, só funções que existem
//! desde o Windows 7), no desenho do pw-launcher (B275 do pw-universal). Tudo o que fala com o
//! servidor ou espera o jogo roda numa thread à parte e devolve o resultado à janela com
//! `PostMessageW` (um `Box` no `LPARAM`); o estado da janela vive só na thread dela.
//!
//! Único módulo com `unsafe`: as chamadas Win32. Cada bloco só passa ponteiros de buffers vivos no
//! próprio bloco e handles criados por esta janela.
use crate::api::{Api, ApiError, Session};
use crate::config::{Config, normalize_server};
use crate::game;
use crate::selfupdate;
use crate::update;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::PCWSTR;
use zeroize::{Zeroize, Zeroizing};

const TITLE: &str = "Vindictus";

const ID_PANEL: i32 = 100;
const ID_REGISTER: i32 = 101;
const ID_USER: i32 = 110;
const ID_PASSWORD: i32 = 111;
const ID_LOGIN: i32 = 112;
const ID_LOGOUT: i32 = 113;
const ID_FOLDER: i32 = 120;
const ID_PLAY: i32 = 130;
const ID_ADMIN: i32 = 131;
const ID_UPDATE: i32 = 132;
const ID_VERIFY: i32 = 133;
const ID_SERVER: i32 = 140;
const ID_SAVE: i32 = 141;

const WM_STATUS: u32 = WM_APP + 1;
const WM_LOGGED_IN: u32 = WM_APP + 2;
const WM_GAME_STARTED: u32 = WM_APP + 3;
const WM_GAME_EXITED: u32 = WM_APP + 4;
const WM_LOGGED_OUT: u32 = WM_APP + 5;
const WM_REMOTE: u32 = WM_APP + 6;
const WM_PROGRESS: u32 = WM_APP + 7;
const WM_UPDATED: u32 = WM_APP + 8;
const WM_NEWS: u32 = WM_APP + 9;
const WM_LAUNCHER: u32 = WM_APP + 10;

const TIMER_STATUS: usize = 1;
const TIMER_CLOSE: usize = 2;
const STATUS_EVERY_MS: u32 = 30_000;
/// Fechar não espera mais que isso pelo logout.
const CLOSE_WAIT_MS: u32 = 4_000;

// Estilos que a windows-sys 0.52 não exporta no módulo usado (winuser.h).
const SS_NOPREFIX: u32 = 0x0080;
const SS_ETCHEDHORZ: u32 = 0x0010;
const SS_PATHELLIPSIS: u32 = 0x8000;
const SS_EDITCONTROL: u32 = 0x2000;
// winuser.h: DM_GETDEFID = WM_USER + 0, DC_HASDEFID = 0x534B.
const DM_GETDEFID: u32 = WM_USER;
const DC_HASDEFID: isize = 0x534B;

#[derive(Clone, Copy, Default)]
struct Controls {
    window: HWND,
    status: HWND,
    news: HWND,
    user_label: HWND,
    user: HWND,
    password_label: HWND,
    password: HWND,
    login: HWND,
    connected: HWND,
    logout: HWND,
    folder: HWND,
    choose_folder: HWND,
    play: HWND,
    admin: HWND,
    version: HWND,
    update: HWND,
    verify: HWND,
    progress: HWND,
    message: HWND,
    server: HWND,
    save: HWND,
    dpi: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Normal,
    Ok,
    Error,
}

#[derive(Default)]
struct State {
    config: Config,
    session: Option<Arc<Session>>,
    user: String,
    busy: bool,
    game_running: bool,
    closing: bool,
    online: Option<bool>,
    /// Versão do cliente na pasta (`.vindictus-update/estado.json`); `None` sem estado.
    local: Option<u32>,
    /// Versão publicada e mínima (`versao.json` assinado); `None` sem resposta.
    remote: Option<(u32, u32)>,
    /// Instalando, atualizando ou verificando; o flag cancela.
    updating: Option<Arc<AtomicBool>>,
    /// Espelho de `game_running` para a thread da atualização.
    game_flag: Arc<AtomicBool>,
    /// Versão do launcher já baixada e conferida, à espera da troca.
    launcher_staged: Option<String>,
}

thread_local! {
    static CTL: Cell<Controls> = Cell::new(Controls::default());
    static ST: RefCell<State> = RefCell::new(State::default());
    static TONE: Cell<Tone> = const { Cell::new(Tone::Normal) };
}

fn ctl() -> Controls {
    CTL.with(Cell::get)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Tamanho de projeto (96 dpi) na escala da tela.
fn px(v: i32) -> i32 {
    v * ctl().dpi / 96
}

fn set_text(h: HWND, t: &str) {
    unsafe { SetWindowTextW(h, wide(t).as_ptr()) };
}

/// Texto de um controle, apagando o buffer intermediário.
fn read_text(h: HWND) -> Zeroizing<String> {
    unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut buffer = Zeroizing::new(vec![0u16; n + 1]);
        let read = GetWindowTextW(h, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
        Zeroizing::new(String::from_utf16_lossy(&buffer[..read]))
    }
}

fn show(h: HWND, visible: bool) {
    unsafe { ShowWindow(h, if visible { SW_SHOW } else { SW_HIDE }) };
}

fn enable(h: HWND, on: bool) {
    unsafe { EnableWindow(h, on as BOOL) };
}

fn error_box(text: &str) {
    unsafe {
        MessageBoxW(
            ctl().window,
            wide(text).as_ptr(),
            wide(TITLE).as_ptr(),
            MB_OK | MB_ICONERROR,
        )
    };
}

/// Manda `value` para a janela; se ela já fechou, o valor é descartado.
fn post<T>(window: HWND, message: u32, value: T) {
    let pointer = Box::into_raw(Box::new(value));
    if unsafe { PostMessageW(window, message, 0, pointer as LPARAM) } == 0 {
        drop(unsafe { Box::from_raw(pointer) });
    }
}

/// O `LPARAM` de uma mensagem mandada por `post` com o mesmo tipo.
unsafe fn take<T>(lparam: LPARAM) -> T {
    *unsafe { Box::from_raw(lparam as *mut T) }
}

// A windows-sys 0.52 prende estas a um feature que ela não publica; declaradas como no
// shlobj_core.h e shellapi.h (shell32.dll, presentes desde o Windows Vista).
#[repr(C)]
struct BrowseInfo {
    hwnd_owner: HWND,
    pidl_root: *const core::ffi::c_void,
    display_name: *mut u16,
    title: PCWSTR,
    flags: u32,
    callback: Option<unsafe extern "system" fn(HWND, u32, LPARAM, LPARAM) -> i32>,
    lparam: LPARAM,
    image: i32,
}
const BIF_RETURNONLYFSDIRS: u32 = 0x0001;
const BIF_EDITBOX: u32 = 0x0010;
const BIF_NEWDIALOGSTYLE: u32 = 0x0040;
// shellapi.h: SHELLEXECUTEINFOW (a união hIcon/hMonitor é um HANDLE).
#[repr(C)]
struct ShellExecuteInfo {
    size: u32,
    mask: u32,
    hwnd: HWND,
    verb: PCWSTR,
    file: PCWSTR,
    parameters: PCWSTR,
    directory: PCWSTR,
    show: i32,
    instance: HINSTANCE,
    id_list: *mut core::ffi::c_void,
    class: PCWSTR,
    key_class: isize,
    hot_key: u32,
    icon_or_monitor: HANDLE,
    process: HANDLE,
}
const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHBrowseForFolderW(info: *const BrowseInfo) -> *mut core::ffi::c_void;
    fn SHGetPathFromIDListW(pidl: *const core::ffi::c_void, path: *mut u16) -> BOOL;
    fn ILFree(pidl: *const core::ffi::c_void);
    fn ShellExecuteExW(info: *mut ShellExecuteInfo) -> BOOL;
    fn ShellExecuteW(
        hwnd: HWND,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

// ---------------------------------------------------------------- tela

fn message(text: &str, tone: Tone) {
    TONE.with(|t| t.set(tone));
    let h = ctl().message;
    set_text(h, text);
    unsafe { InvalidateRect(h, std::ptr::null(), 1) };
}

fn api() -> Result<Api, String> {
    ST.with(|s| s.borrow().config.server_base())
        .map(|base| Api::new(&base))
}

/// Mostra os controles conforme o estado (fora, entrando, conectado, jogando, fechando).
fn refresh() {
    let c = ctl();
    ST.with(|s| {
        let s = s.borrow();
        let logged = s.session.is_some();
        for h in [c.user_label, c.user, c.password_label, c.password, c.login] {
            show(h, !logged);
        }
        show(c.connected, logged);
        show(c.logout, logged);
        set_text(c.connected, &format!("Conectado como {}", s.user));
        let idle = !s.busy && !s.closing;
        for h in [c.user, c.password, c.login, c.server, c.save, c.admin] {
            enable(h, idle);
        }
        enable(c.choose_folder, idle && s.updating.is_none());
        enable(c.logout, idle && !s.game_running);
        let folder_ok = game::is_game_folder(Path::new(&s.config.game_folder));
        let updating = s.updating.is_some();
        // Abaixo da mínima publicada o jogo não abre; sem estado (cópia antiga) abre, e "Verificar
        // arquivos" registra a versão.
        let below_minimum =
            matches!((s.local, s.remote), (Some(local), Some((_, minimum))) if local < minimum);
        enable(
            c.play,
            idle && logged && !s.game_running && folder_ok && !updating && !below_minimum,
        );
        let has_folder = !s.config.game_folder.is_empty();
        let (update_text, update_on) = if updating {
            ("Cancelar", true)
        } else {
            match (folder_ok, s.local, s.remote) {
                (false, _, Some(_)) if has_folder => ("Instalar o jogo aqui", true),
                (true, Some(local), Some((current, _))) if local < current => ("Atualizar", true),
                (true, None, Some(_)) => ("Já tenho o cliente", true),
                _ => ("Atualizado", false),
            }
        };
        set_text(c.update, update_text);
        enable(
            c.update,
            update_on && (updating || (idle && !s.game_running)),
        );
        enable(
            c.verify,
            idle && !updating && !s.game_running && folder_ok && s.remote.is_some(),
        );
        set_text(
            c.version,
            &match (s.local, s.remote) {
                (Some(l), Some((r, _))) if l >= r => format!("Cliente: versão {l} (atual)"),
                (Some(l), Some((r, _))) => format!("Cliente: versão {l}; publicada: {r}"),
                (None, Some((r, _))) if folder_ok => {
                    format!("Cliente sem registro; publicada: {r}")
                }
                (None, Some((r, _))) => format!("Publicada: versão {r}"),
                (Some(l), None) => format!("Cliente: versão {l}; atualização fora do ar"),
                (None, None) => "Atualização: verificando…".into(),
            },
        );
        set_text(
            c.play,
            if s.game_running {
                "Jogo aberto"
            } else {
                "Jogar"
            },
        );
        set_text(
            c.folder,
            if s.config.game_folder.is_empty() {
                "(escolha a pasta do jogo)"
            } else {
                &s.config.game_folder
            },
        );
        set_text(
            c.status,
            match s.online {
                None => "Servidor: verificando…",
                Some(true) => "Servidor: online",
                Some(false) => "Servidor: fora do ar",
            },
        );
    });
}

fn set_busy(on: bool) {
    ST.with(|s| s.borrow_mut().busy = on);
    refresh();
}

// ---------------------------------------------------------------- ações

/// Versão local (estado na pasta) e a publicada (assinatura conferida) numa thread.
fn check_versions() {
    let (folder, base) = ST.with(|s| {
        let s = s.borrow();
        (
            PathBuf::from(&s.config.game_folder),
            s.config.updates_base(),
        )
    });
    let local = (!folder.as_os_str().is_empty())
        .then(|| update::local_state(&folder))
        .flatten();
    ST.with(|s| s.borrow_mut().local = local.map(|l| l.version));
    refresh();
    let Ok(base) = base else { return };
    let window = ctl().window;
    let staged = ST.with(|s| s.borrow().launcher_staged.is_some());
    std::thread::spawn(move || {
        let fetch = update::HttpFetch::new(&base);
        let key = update::pinned_key();
        let pointer = key.as_ref().map_err(|e| e.message()).and_then(|key| {
            update::remote(&fetch, key)
                .map(|r| r.pointer)
                .map_err(|e| e.message())
        });
        post(
            window,
            WM_REMOTE,
            pointer
                .as_ref()
                .map(|p| (p.current, p.minimum))
                .map_err(Clone::clone),
        );
        let Ok(key) = key else { return };
        if let Ok(items) = update::news(&fetch, &key) {
            post(window, WM_NEWS, items);
        }
        // O launcher publicado: baixado uma vez, trocado quando der.
        if let (Ok(pointer), Some(exe), false) = (pointer, selfupdate::current_exe(), staged)
            && let Some(launcher) = selfupdate::wanted(&pointer, &exe)
        {
            let result = selfupdate::download(&fetch, &launcher, &exe, &AtomicBool::new(false))
                .map(|()| launcher.version)
                .map_err(|e| e.message());
            post(window, WM_LAUNCHER, result);
        }
    });
}

/// Data (dd/mm/aaaa) de segundos desde 1970, em UTC (algoritmo civil de Howard Hinnant).
fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day:02}/{month:02}/{year}")
}

fn on_news(items: Vec<vindictus_client_dist::manifest::NewsItem>) {
    let text = if items.is_empty() {
        "Sem notícias.".to_owned()
    } else {
        items
            .iter()
            .map(|n| {
                format!(
                    "{}  {}\r\n{}",
                    date(n.published_unix),
                    n.title,
                    n.body.replace("\r\n", "\n").replace('\n', "\r\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\r\n\r\n")
    };
    set_text(ctl().news, &text);
}

/// Troca o exe e abre o novo; `false` se não deu (fica para a próxima abertura).
fn restart_into_new() -> bool {
    let Some(exe) = selfupdate::current_exe() else {
        return false;
    };
    match selfupdate::swap(&exe) {
        Ok(true) if selfupdate::restart(&exe) => {
            unsafe { DestroyWindow(ctl().window) };
            true
        }
        _ => false,
    }
}

fn on_launcher(result: Result<String, String>) {
    let Ok(version) = result else { return };
    let idle = ST.with(|s| {
        let s = s.borrow();
        s.session.is_none() && !s.busy && s.updating.is_none() && !s.game_running && !s.closing
    });
    ST.with(|s| s.borrow_mut().launcher_staged = Some(version.clone()));
    if idle && restart_into_new() {
        return;
    }
    message(
        &format!("Launcher {version} baixado: entra na próxima vez que abrir o launcher."),
        Tone::Ok,
    );
}

fn on_remote(remote: Result<(u32, u32), String>) {
    ST.with(|s| s.borrow_mut().remote = remote.ok());
    refresh();
}

/// Instalar, "já tenho o cliente", atualizar ou verificar (`full`); com uma em curso, cancela.
fn on_update(full: bool) {
    if let Some(cancel) = ST.with(|s| s.borrow().updating.clone()) {
        cancel.store(true, Ordering::Relaxed);
        message("Cancelando…", Tone::Normal);
        return;
    }
    let (folder, base, game_flag) = ST.with(|s| {
        let s = s.borrow();
        (
            PathBuf::from(&s.config.game_folder),
            s.config.updates_base(),
            s.game_flag.clone(),
        )
    });
    let base = match base {
        Ok(base) => base,
        Err(e) => return message(&e, Tone::Error),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    ST.with(|s| s.borrow_mut().updating = Some(cancel.clone()));
    set_progress(0, 1);
    message("Preparando…", Tone::Normal);
    refresh();
    let window = ctl().window;
    let mode = if full {
        update::Mode::Full
    } else {
        update::Mode::Changes
    };
    std::thread::spawn(move || {
        let fetch = update::HttpFetch::new(&base);
        let last = std::cell::Cell::new((String::new(), u64::MAX));
        let report = |p: update::Progress| {
            // No máximo uma mensagem por milésimo e por arquivo novo.
            let step = (p.done * 1000).checked_div(p.total).unwrap_or(0);
            let (file, shown) = last.take();
            if step != shown || file != p.file {
                post(window, WM_PROGRESS, p.clone());
            }
            last.set((p.file, step));
        };
        let running = || game_flag.load(Ordering::Relaxed);
        let result = update::pinned_key()
            .and_then(|key| {
                update::run(
                    &folder, &fetch, &key, mode, None, &cancel, &running, &report,
                )
            })
            .and_then(|outcome| {
                // O cliente procura o jogo no host do servidor de atualização.
                if let Some(url) = update::bootstrap_url(&base) {
                    update::apply_endpoint(&folder, &url)?;
                }
                Ok(outcome)
            })
            .map_err(|e| e.message());
        post(window, WM_UPDATED, result);
    });
}

fn set_progress(done: u64, total: u64) {
    let position = (done * 1000).checked_div(total).unwrap_or(0).min(1000);
    unsafe { SendMessageW(ctl().progress, PBM_SETPOS, position as WPARAM, 0) };
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

fn on_progress(p: update::Progress) {
    set_progress(p.done, p.total);
    let file = p.file.rsplit('/').next().unwrap_or(&p.file).to_owned();
    message(
        &format!(
            "{}: {} de {}  {file}",
            p.phase,
            megabytes(p.done),
            megabytes(p.total)
        ),
        Tone::Normal,
    );
}

fn on_updated(result: Result<update::Outcome, String>) {
    ST.with(|s| s.borrow_mut().updating = None);
    match result {
        Ok(o) => {
            set_progress(1, 1);
            message(
                &format!(
                    "Cliente na versão {}: {} arquivos conferidos, {} baixados ({}), {} por delta.",
                    o.version,
                    o.checked,
                    o.downloaded,
                    megabytes(o.downloaded_bytes),
                    o.deltas
                ),
                Tone::Ok,
            );
        }
        Err(e) => message(&e, Tone::Error),
    }
    check_versions();
}

fn check_status() {
    let Ok(api) = api() else {
        ST.with(|s| s.borrow_mut().online = Some(false));
        refresh();
        return;
    };
    let window = ctl().window;
    std::thread::spawn(move || post(window, WM_STATUS, api.online()));
}

fn on_login() {
    let c = ctl();
    let user = read_text(c.user).trim().to_owned();
    let password = read_text(c.password);
    if user.is_empty() || password.is_empty() {
        message("Digite o login e a senha.", Tone::Error);
        return;
    }
    let api = match api() {
        Ok(api) => api,
        Err(e) => return message(&e, Tone::Error),
    };
    set_busy(true);
    message("Entrando…", Tone::Normal);
    let window = c.window;
    std::thread::spawn(move || {
        let result = api.login(&user, &password).map(|session| (session, user));
        drop(password);
        post(window, WM_LOGGED_IN, result);
    });
}

fn on_logged_in(result: Result<(Session, String), ApiError>) {
    let c = ctl();
    set_text(c.password, "");
    match result {
        Ok((session, user)) => {
            ST.with(|s| {
                let mut s = s.borrow_mut();
                s.session = Some(Arc::new(session));
                s.config.last_login = user.clone();
                s.user = user;
                s.online = Some(true);
                let _ = s.config.save();
            });
            message(
                "Pronto. O login vale enquanto o launcher estiver aberto.",
                Tone::Ok,
            );
        }
        Err(e) => {
            if e == ApiError::Offline {
                ST.with(|s| s.borrow_mut().online = Some(false));
            }
            message(&e.message(), Tone::Error);
            unsafe { SetFocus(c.password) };
        }
    }
    set_busy(false);
}

/// The running game: a child of the launcher, or an elevated process (handle of `ShellExecuteExW`).
enum Running {
    Child(std::process::Child),
    Elevated(HANDLE),
}

impl Running {
    fn wait(self) {
        match self {
            Running::Child(mut child) => {
                let _ = child.wait();
            }
            Running::Elevated(handle) => unsafe {
                WaitForSingleObject(handle, INFINITE);
                CloseHandle(handle);
            },
        }
    }
}

/// Modo administrador: o Windows pede o UAC só para o jogo; o launcher continua sem elevação.
fn start_elevated(folder: &Path, ticket: &str) -> Result<Running, String> {
    game::check_folder(folder)?;
    let file = wide(&folder.join(game::EXECUTABLE).to_string_lossy());
    let directory = wide(&folder.to_string_lossy());
    let parameters = Zeroizing::new(wide(&game::command_line(ticket)));
    let verb = wide("runas");
    let mut info = ShellExecuteInfo {
        size: std::mem::size_of::<ShellExecuteInfo>() as u32,
        mask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: 0,
        verb: verb.as_ptr(),
        file: file.as_ptr(),
        parameters: parameters.as_ptr(),
        directory: directory.as_ptr(),
        show: SW_SHOWNORMAL,
        instance: 0,
        id_list: std::ptr::null_mut(),
        class: std::ptr::null(),
        key_class: 0,
        hot_key: 0,
        icon_or_monitor: 0,
        process: 0,
    };
    unsafe {
        // ShellExecuteEx quer COM iniciado na thread que o chama.
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if ShellExecuteExW(&mut info) == 0 || info.process == 0 {
            return Err(
                "O jogo não foi aberto como administrador (permissão negada ou cancelada).".into(),
            );
        }
    }
    Ok(Running::Elevated(info.process))
}

fn on_play() {
    let (session, folder, as_admin) = ST.with(|s| {
        let s = s.borrow();
        (
            s.session.clone(),
            PathBuf::from(&s.config.game_folder),
            s.config.game_as_admin,
        )
    });
    let Some(session) = session else { return };
    let api = match api() {
        Ok(api) => api,
        Err(e) => return message(&e, Tone::Error),
    };
    let server = ST
        .with(|s| s.borrow().config.server_base())
        .unwrap_or_default();
    let bootstrap = ST
        .with(|s| s.borrow().config.updates_base())
        .ok()
        .and_then(|base| update::bootstrap_url(&base));
    set_busy(true);
    message("Abrindo o jogo…", Tone::Normal);
    let window = ctl().window;
    std::thread::spawn(move || {
        // O endereço do servidor no cliente segue o servidor de atualização configurado agora.
        let endpoint = bootstrap
            .map(|url| update::apply_endpoint(&folder, &url).map_err(|e| (false, e.message())))
            .transpose();
        let started = endpoint
            .and_then(|_| {
                api.ticket(&session)
                    .map_err(|e| (e == ApiError::SessionEnded, e.message()))
            })
            .and_then(|ticket| {
                game::write_messenger_ini(&folder, &server);
                if as_admin {
                    start_elevated(&folder, ticket.as_str())
                } else {
                    game::start(&folder, ticket.as_str()).map(Running::Child)
                }
                .map_err(|e| (false, e))
            });
        match started {
            Ok(running) => {
                post(window, WM_GAME_STARTED, Ok::<(), (bool, String)>(()));
                running.wait();
                post(window, WM_GAME_EXITED, ());
            }
            Err(e) => post(window, WM_GAME_STARTED, Err::<(), (bool, String)>(e)),
        }
    });
}

fn on_toggle_admin() {
    let checked = unsafe { SendMessageW(ctl().admin, BM_GETCHECK, 0, 0) } == BST_CHECKED as LRESULT;
    let saved = ST.with(|s| {
        let mut s = s.borrow_mut();
        s.config.game_as_admin = checked;
        s.config.save()
    });
    match saved {
        Ok(()) if checked => message(
            "O jogo vai abrir como administrador: o Windows pede permissão a cada Jogar.",
            Tone::Normal,
        ),
        Ok(()) => message("O jogo vai abrir sem pedir administrador.", Tone::Normal),
        Err(e) => message(&e, Tone::Error),
    }
}

fn on_game_started(result: Result<(), (bool, String)>) {
    match result {
        Ok(()) => {
            ST.with(|s| {
                let mut s = s.borrow_mut();
                s.game_running = true;
                s.game_flag.store(true, Ordering::Relaxed);
            });
            message(
                "Jogo aberto. Clique em Start Game quando quiser: o login vale enquanto o launcher estiver aberto.",
                Tone::Ok,
            );
        }
        Err((session_ended, text)) => {
            if session_ended {
                ST.with(|s| s.borrow_mut().session = None);
            }
            message(&text, Tone::Error);
        }
    }
    set_busy(false);
}

fn on_game_exited() {
    ST.with(|s| {
        let mut s = s.borrow_mut();
        s.game_running = false;
        s.game_flag.store(false, Ordering::Relaxed);
    });
    message("Jogo fechado.", Tone::Normal);
    refresh();
}

/// Encerra a sessão numa thread; `WM_LOGGED_OUT` volta quando terminar.
fn start_logout() -> bool {
    let session = ST.with(|s| s.borrow_mut().session.take());
    let (Some(session), Ok(api)) = (session, api()) else {
        return false;
    };
    let window = ctl().window;
    std::thread::spawn(move || {
        let _ = api.logout(&session);
        post(window, WM_LOGGED_OUT, ());
    });
    true
}

fn on_logout_clicked() {
    set_busy(true);
    if !start_logout() {
        set_busy(false);
    }
}

fn on_logged_out() {
    let closing = ST.with(|s| s.borrow().closing);
    if closing {
        unsafe { DestroyWindow(ctl().window) };
        return;
    }
    message("Você saiu.", Tone::Normal);
    set_busy(false);
}

fn on_close() {
    let (closing, game_running, logged) = ST.with(|s| {
        let s = s.borrow();
        (s.closing, s.game_running, s.session.is_some())
    });
    if closing {
        unsafe { DestroyWindow(ctl().window) };
        return;
    }
    if let Some(cancel) = ST.with(|s| s.borrow().updating.clone()) {
        let answer = unsafe {
            MessageBoxW(
                ctl().window,
                wide("A atualização está em andamento. Ela continua de onde parou na próxima vez.\n\nFechar o launcher?").as_ptr(),
                wide(TITLE).as_ptr(),
                MB_YESNO | MB_ICONQUESTION,
            )
        };
        if answer != IDYES {
            return;
        }
        cancel.store(true, Ordering::Relaxed);
    }
    if game_running && logged {
        let answer = unsafe {
            MessageBoxW(
                ctl().window,
                wide("O jogo está aberto. Fechar o launcher encerra o login: se você ainda não clicou em Start Game, terá de abrir o jogo de novo pelo launcher.\n\nFechar o launcher?").as_ptr(),
                wide(TITLE).as_ptr(),
                MB_YESNO | MB_ICONQUESTION,
            )
        };
        if answer != IDYES {
            return;
        }
    }
    ST.with(|s| s.borrow_mut().closing = true);
    refresh();
    if start_logout() {
        unsafe { SetTimer(ctl().window, TIMER_CLOSE, CLOSE_WAIT_MS, None) };
    } else {
        unsafe { DestroyWindow(ctl().window) };
    }
}

fn choose_folder() -> Option<PathBuf> {
    let mut name = [0u16; 260];
    let title = wide("Escolha a pasta do jogo (a que tem o Vindictus.exe):");
    let info = BrowseInfo {
        hwnd_owner: ctl().window,
        pidl_root: std::ptr::null(),
        display_name: name.as_mut_ptr(),
        title: title.as_ptr(),
        flags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
        callback: None,
        lparam: 0,
        image: 0,
    };
    unsafe {
        let pidl = SHBrowseForFolderW(&info);
        if pidl.is_null() {
            return None;
        }
        let mut path = [0u16; 260];
        let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
        ILFree(pidl);
        if ok == 0 {
            return None;
        }
        let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        Some(PathBuf::from(String::from_utf16_lossy(&path[..end])))
    }
}

fn on_choose_folder() {
    let Some(folder) = choose_folder() else {
        return;
    };
    let is_game = game::is_game_folder(&folder);
    let saved = ST.with(|s| {
        let mut s = s.borrow_mut();
        s.config.game_folder = folder.to_string_lossy().into_owned();
        s.config.save()
    });
    match saved {
        Ok(()) if is_game => message("Pasta do jogo salva.", Tone::Ok),
        Ok(()) => message(
            "Essa pasta ainda não tem o jogo: \"Instalar o jogo aqui\" baixa o cliente nela.",
            Tone::Normal,
        ),
        Err(e) => message(&e, Tone::Error),
    }
    refresh();
    check_versions();
}

fn on_save_server() {
    let text = read_text(ctl().server);
    let base = match normalize_server(&text) {
        Ok(base) => base,
        Err(e) => return message(&e, Tone::Error),
    };
    let saved = ST.with(|s| {
        let mut s = s.borrow_mut();
        let changed = s.config.server != base;
        s.config.server = base.clone();
        if changed {
            s.session = None;
            s.online = None;
        }
        s.config.save()
    });
    set_text(ctl().server, &base);
    match saved {
        Ok(()) => message("Servidor salvo.", Tone::Ok),
        Err(e) => message(&e, Tone::Error),
    }
    refresh();
    check_status();
    check_versions();
}

fn open_url(url: &str) {
    unsafe {
        ShellExecuteW(
            ctl().window,
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
}

fn on_open(register: bool) {
    match api() {
        Ok(api) => open_url(&if register {
            api.register_url()
        } else {
            api.panel_url()
        }),
        Err(e) => message(&e, Tone::Error),
    }
}

// ---------------------------------------------------------------- janela

unsafe extern "system" fn procedure(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;
            if code == BN_CLICKED {
                match id {
                    ID_LOGIN => on_login(),
                    ID_LOGOUT => on_logout_clicked(),
                    ID_PLAY => on_play(),
                    ID_ADMIN => on_toggle_admin(),
                    ID_UPDATE => on_update(false),
                    ID_VERIFY => on_update(true),
                    ID_FOLDER => on_choose_folder(),
                    ID_SAVE => on_save_server(),
                    ID_PANEL => on_open(false),
                    ID_REGISTER => on_open(true),
                    _ => {}
                }
            }
            0
        }
        // Enter aperta Entrar (fora) ou Jogar (conectado), como num diálogo.
        DM_GETDEFID => {
            let (logged, idle) = ST.with(|s| {
                let s = s.borrow();
                (
                    s.session.is_some(),
                    !s.busy && !s.closing && !s.game_running,
                )
            });
            if !idle {
                0
            } else {
                (DC_HASDEFID << 16) | if logged { ID_PLAY } else { ID_LOGIN } as isize
            }
        }
        WM_CTLCOLORSTATIC => {
            let hdc = wparam as HDC;
            unsafe {
                SetBkColor(hdc, GetSysColor(COLOR_BTNFACE));
                let color = if lparam == ctl().message {
                    match TONE.with(Cell::get) {
                        Tone::Normal => GetSysColor(COLOR_WINDOWTEXT),
                        Tone::Ok => 0x0000_7A1E, // verde (COLORREF é 0x00BBGGRR)
                        Tone::Error => 0x0000_00C0, // vermelho
                    }
                } else if lparam == ctl().status {
                    match ST.with(|s| s.borrow().online) {
                        Some(true) => 0x0000_7A1E,
                        Some(false) => 0x0000_00C0,
                        None => GetSysColor(COLOR_GRAYTEXT),
                    }
                } else {
                    GetSysColor(COLOR_WINDOWTEXT)
                };
                SetTextColor(hdc, color);
                GetSysColorBrush(COLOR_BTNFACE)
            }
        }
        WM_TIMER => {
            match wparam {
                TIMER_STATUS => {
                    check_status();
                    if ST.with(|s| s.borrow().updating.is_none()) {
                        check_versions();
                    }
                }
                TIMER_CLOSE => unsafe {
                    DestroyWindow(window);
                },
                _ => {}
            }
            0
        }
        WM_STATUS => {
            let online: bool = unsafe { take(lparam) };
            ST.with(|s| s.borrow_mut().online = Some(online));
            refresh();
            0
        }
        WM_LOGGED_IN => {
            on_logged_in(unsafe { take(lparam) });
            0
        }
        WM_GAME_STARTED => {
            on_game_started(unsafe { take(lparam) });
            0
        }
        WM_GAME_EXITED => {
            let () = unsafe { take(lparam) };
            on_game_exited();
            0
        }
        WM_LOGGED_OUT => {
            let () = unsafe { take(lparam) };
            on_logged_out();
            0
        }
        WM_REMOTE => {
            on_remote(unsafe { take(lparam) });
            0
        }
        WM_PROGRESS => {
            on_progress(unsafe { take(lparam) });
            0
        }
        WM_UPDATED => {
            on_updated(unsafe { take(lparam) });
            0
        }
        WM_NEWS => {
            on_news(unsafe { take(lparam) });
            0
        }
        WM_LAUNCHER => {
            on_launcher(unsafe { take(lparam) });
            0
        }
        WM_CLOSE => {
            on_close();
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, msg, wparam, lparam) },
    }
}

fn font(points: i32, bold: bool, dpi: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -(points * dpi / 72),
            0,
            0,
            0,
            if bold { 700 } else { 400 },
            0,
            0,
            0,
            1, // DEFAULT_CHARSET
            0,
            0,
            5, // CLEARTYPE_QUALITY
            0,
            wide("Segoe UI").as_ptr(),
        )
    }
}

struct Builder {
    window: HWND,
    instance: HINSTANCE,
}

impl Builder {
    #[allow(clippy::too_many_arguments)]
    fn make(
        &self,
        class: &str,
        text: &str,
        style: u32,
        ex: u32,
        r: (i32, i32, i32, i32),
        id: i32,
        f: HFONT,
    ) -> HWND {
        unsafe {
            let h = CreateWindowExW(
                ex,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                px(r.0),
                px(r.1),
                px(r.2),
                px(r.3),
                self.window,
                id as HMENU,
                self.instance,
                std::ptr::null(),
            );
            SendMessageW(h, WM_SETFONT, f as WPARAM, 1);
            h
        }
    }
}

pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        error_box(&format!("Erro interno do launcher:\n{info}"))
    }));
    if let Some(exe) = selfupdate::current_exe() {
        selfupdate::cleanup(&exe);
        // Um launcher novo baixado na última execução entra antes da janela abrir.
        if matches!(selfupdate::swap(&exe), Ok(true)) && selfupdate::restart(&exe) {
            return;
        }
    }
    let mut config = Config::load();
    config.server = config.server_base().unwrap_or(config.server);
    let last_login = config.last_login.clone();
    let server = config.server.clone();
    let as_admin = config.game_as_admin;
    ST.with(|s| {
        s.borrow_mut().config = config;
    });
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let iccx = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_PROGRESS_CLASS,
        };
        InitCommonControlsEx(&iccx);

        let screen = GetDC(0);
        let dpi = GetDeviceCaps(screen, LOGPIXELSY as i32).max(96);
        ReleaseDC(0, screen);
        let normal = font(9, false, dpi);
        let header = font(16, true, dpi);
        let big = font(12, true, dpi);
        CTL.with(|c| {
            c.set(Controls {
                dpi,
                ..Controls::default()
            })
        });

        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("VindictusLauncher");
        let icon = LoadIconW(instance, 1 as PCWSTR); // recurso 1 (launcher.rc)
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(procedure),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(0, IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: icon,
        };
        if RegisterClassExW(&wc) == 0 {
            error_box("Não foi possível registrar a janela do launcher.");
            return;
        }
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: px(520),
            bottom: px(500),
        };
        AdjustWindowRect(&mut r, style, 0);
        let (width, height) = (r.right - r.left, r.bottom - r.top);
        let x = (GetSystemMetrics(SM_CXSCREEN) - width).max(0) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - height).max(0) / 2;
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(TITLE).as_ptr(),
            style,
            x,
            y,
            width,
            height,
            0,
            0,
            instance,
            std::ptr::null(),
        );
        if window == 0 {
            error_box("Não foi possível abrir a janela do launcher.");
            return;
        }

        let b = Builder { window, instance };
        let label = |t: &str, f: HFONT, r: (i32, i32, i32, i32), extra: u32| {
            b.make("STATIC", t, SS_NOPREFIX | extra, 0, r, -1, f)
        };
        let button = |t: &str, r: (i32, i32, i32, i32), id: i32, kind: i32, f: HFONT| {
            b.make("BUTTON", t, kind as u32 | WS_TABSTOP, 0, r, id, f)
        };
        let edit = |t: &str, r: (i32, i32, i32, i32), id: i32, extra: i32| {
            b.make(
                "EDIT",
                t,
                (ES_AUTOHSCROLL | extra) as u32 | WS_TABSTOP,
                WS_EX_CLIENTEDGE,
                r,
                id,
                normal,
            )
        };

        label("Vindictus", header, (16, 12, 200, 34), 0);
        button(
            "Painel",
            (300, 16, 100, 28),
            ID_PANEL,
            BS_PUSHBUTTON,
            normal,
        );
        button(
            "Criar conta",
            (404, 16, 100, 28),
            ID_REGISTER,
            BS_PUSHBUTTON,
            normal,
        );
        let status = label("Servidor: verificando…", normal, (16, 54, 488, 20), 0);
        let news = b.make(
            "EDIT",
            "Notícias: verificando…",
            (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL | WS_TABSTOP,
            WS_EX_CLIENTEDGE,
            (16, 80, 488, 76),
            -1,
            normal,
        );
        label("", normal, (16, 164, 488, 2), SS_ETCHEDHORZ);

        let user_label = label("Login:", normal, (16, 182, 60, 20), 0);
        let user = edit(&last_login, (80, 178, 180, 24), ID_USER, 0);
        let password_label = label("Senha:", normal, (16, 212, 60, 20), 0);
        let password = edit("", (80, 208, 180, 24), ID_PASSWORD, ES_PASSWORD);
        let login = button(
            "Entrar",
            (272, 192, 100, 30),
            ID_LOGIN,
            BS_DEFPUSHBUTTON,
            normal,
        );
        let connected = label("", big, (16, 188, 360, 26), 0);
        let logout = button(
            "Sair",
            (404, 186, 100, 30),
            ID_LOGOUT,
            BS_PUSHBUTTON,
            normal,
        );

        label("Pasta do jogo:", normal, (16, 252, 90, 20), 0);
        let folder = label("", normal, (108, 252, 300, 20), SS_PATHELLIPSIS);
        let choose_folder = button(
            "Escolher…",
            (412, 246, 92, 28),
            ID_FOLDER,
            BS_PUSHBUTTON,
            normal,
        );
        let version = label("Atualização: verificando…", normal, (16, 284, 280, 20), 0);
        let verify = button(
            "Verificar arquivos",
            (300, 278, 100, 28),
            ID_VERIFY,
            BS_PUSHBUTTON,
            normal,
        );
        let update_button = button(
            "Atualizado",
            (404, 278, 100, 28),
            ID_UPDATE,
            BS_PUSHBUTTON,
            normal,
        );
        let progress = b.make(
            "msctls_progress32",
            "",
            0,
            0,
            (16, 314, 488, 16),
            -1,
            normal,
        );
        SendMessageW(progress, PBM_SETRANGE32, 0, 1000);
        let play = button("Jogar", (16, 338, 488, 42), ID_PLAY, BS_PUSHBUTTON, big);
        let admin = button(
            "Abrir o jogo como administrador (só se ele não abrir sem)",
            (16, 388, 488, 22),
            ID_ADMIN,
            BS_AUTOCHECKBOX,
            normal,
        );
        if as_admin {
            SendMessageW(admin, BM_SETCHECK, BST_CHECKED as WPARAM, 0);
        }
        let message = label("", normal, (16, 414, 488, 36), SS_EDITCONTROL);
        label("", normal, (16, 456, 488, 2), SS_ETCHEDHORZ);
        label("Servidor:", normal, (16, 471, 60, 20), 0);
        let server_edit = edit(&server, (80, 467, 324, 24), ID_SERVER, 0);
        let save = button("Salvar", (412, 466, 92, 26), ID_SAVE, BS_PUSHBUTTON, normal);

        CTL.with(|c| {
            c.set(Controls {
                window,
                status,
                news,
                user_label,
                user,
                password_label,
                password,
                login,
                connected,
                logout,
                folder,
                choose_folder,
                play,
                admin,
                version,
                update: update_button,
                verify,
                progress,
                message,
                server: server_edit,
                save,
                dpi,
            })
        });
        refresh();
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        SetFocus(if last_login.is_empty() {
            user
        } else {
            password
        });
        check_status();
        check_versions();
        SetTimer(window, TIMER_STATUS, STATUS_EVERY_MS, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if IsDialogMessageW(window, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    // A senha digitada e não usada não fica no buffer do controle.
    let mut leftover = read_text(ctl().password).to_string();
    leftover.zeroize();
}
