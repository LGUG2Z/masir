use clap::Parser;
use color_eyre::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use windows::core::Result as WindowsCrateResult;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;
use windows::Win32::UI::Input::KeyboardAndMouse::INPUT;
use windows::Win32::UI::Input::KeyboardAndMouse::INPUT_MOUSE;
use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::Win32::UI::WindowsAndMessaging::GetWindowLongW;
use windows::Win32::UI::WindowsAndMessaging::RealGetWindowClassW;
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
use windows::Win32::UI::WindowsAndMessaging::WindowFromPoint;
use windows::Win32::UI::WindowsAndMessaging::GA_ROOT;
use windows::Win32::UI::WindowsAndMessaging::GET_ANCESTOR_FLAGS;
use windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE;
use windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE;
use windows::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE;
use windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW;
use winput::message_loop;
use winput::message_loop::Event;
use winput::Action;

const CLASS_IGNORELIST: [(&str, MatchingStrategy); 9] = [
    ("SHELLDLL_DefView", MatchingStrategy::Equals), // desktop window
    ("Shell_TrayWnd", MatchingStrategy::Equals),    // tray
    ("TrayNotifyWnd", MatchingStrategy::Equals),    // tray
    ("MSTaskSwWClass", MatchingStrategy::Equals),   // start bar icons
    ("Windows.UI.Core.CoreWindow", MatchingStrategy::Equals), // start menu
    ("XamlExplorerHostIslandWindow", MatchingStrategy::Equals), // task switcher
    ("ForegroundStaging", MatchingStrategy::Equals), // also task switcher
    ("Flow.Launcher", MatchingStrategy::Contains),
    ("PowerToys.PowerLauncher", MatchingStrategy::Contains),
];

#[derive(Debug, PartialEq, Eq)]
enum MatchingStrategy {
    Contains,
    Equals,
}

#[derive(Parser)]
#[clap(author, about, version)]
struct Opts {
    /// Disable automatic integrations with tiling window managers (e.g. komorebi)
    #[clap(long)]
    disable_integrations: bool,
    /// Path to a file with known focus-able HWNDs (e.g. komorebi.hwnd.json)
    #[clap(long)]
    hwnds: Option<PathBuf>,
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let hwnds = match opts.hwnds {
        None if opts.disable_integrations => None,
        None => {
            let hwnds: PathBuf = dirs::data_local_dir()
                .expect("there is no local data directory")
                .join("komorebi")
                .join("komorebi.hwnd.json");

            // TODO: We can add checks for other window managers here

            if hwnds.is_file() {
                Some(hwnds)
            } else {
                None
            }
        }
        Some(hwnds) => {
            if hwnds.is_file() {
                Some(hwnds)
            } else {
                None
            }
        }
    };

    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    )?;

    listen_for_movements(hwnds.clone());

    match hwnds {
        None => tracing::info!("masir is now running"),
        Some(hwnds) => tracing::info!(
            "masir is now running, and additionally checking hwnds against {}",
            hwnds.display()
        ),
    }

    let (ctrlc_sender, ctrlc_receiver) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        ctrlc_sender
            .send(())
            .expect("could not send signal on ctrl-c channel");
    })?;

    ctrlc_receiver
        .recv()
        .expect("could not receive signal on ctrl-c channel");

    tracing::info!("received ctrl-c, exiting");

    Ok(())
}

fn listen_for_movements(hwnds: Option<PathBuf>) {
    std::thread::spawn(move || {
        let receiver = message_loop::start().expect("could not start winput message loop");

        let mut eligibility_cache = HashMap::new();
        let mut class_cache: HashMap<isize, String> = HashMap::new();
        let mut hwnd_pair_cache: HashMap<isize, isize> = HashMap::new();
        let mut root_hwnd_cache: HashMap<isize, isize> = HashMap::new();

        let mut cache_instantiation_time = Instant::now();
        let max_cache_age = Duration::from_secs(60) * 10; // 10 minutes

        let mut is_mouse_down = false;

        loop {
            // clear our caches every 10 minutes
            if cache_instantiation_time.elapsed() > max_cache_age {
                tracing::info!("clearing caches, cache age is >10 minutes");

                eligibility_cache = HashMap::new();
                class_cache = HashMap::new();
                hwnd_pair_cache = HashMap::new();
                root_hwnd_cache = HashMap::new();

                cache_instantiation_time = Instant::now();
            }

            match receiver.next_event() {
                Event::MouseMoveRelative { .. } => {
                    // resizing windows / dragging and dropping files fix
                    if is_mouse_down {
                        continue;
                    }

                    if let (Ok(cursor_pos_hwnd), Ok(foreground_hwnd)) =
                        (window_at_cursor_pos(), foreground_window())
                    {
                        if cursor_pos_hwnd == foreground_hwnd {
                            continue;
                        }

                        let mut cursor_root_hwnd = root_hwnd_cache.get(&cursor_pos_hwnd).cloned();

                        // make syscalls if necessary and populate the root hwnd cache
                        match &cursor_root_hwnd {
                            None => {
                                if let Ok(root_hwnd) = get_ancestor(cursor_pos_hwnd, GA_ROOT) {
                                    root_hwnd_cache.insert(cursor_pos_hwnd, root_hwnd);
                                    cursor_root_hwnd = Some(root_hwnd);
                                }
                            }
                            Some(root_hwnd) => {
                                tracing::debug!(
                                    "hwnd {cursor_pos_hwnd} root hwnd was found in the cache: {root_hwnd}"
                                );
                            }
                        }

                        if let Some(cursor_root_hwnd) = cursor_root_hwnd {
                            if cursor_root_hwnd == foreground_hwnd {
                                continue;
                            }

                            if let Some(paired_hwnd) = hwnd_pair_cache.get(&cursor_root_hwnd) {
                                if *paired_hwnd == foreground_hwnd {
                                    tracing::trace!("hwnds {cursor_root_hwnd} and {foreground_hwnd} are known to refer to the same application, skipping");
                                    continue;
                                }
                            }

                            let mut should_raise = false;

                            // check our class cache to avoid syscalls
                            let mut cursor_root_class = class_cache.get(&cursor_root_hwnd).cloned();
                            let mut foreground_class = class_cache.get(&foreground_hwnd).cloned();

                            // make syscalls if necessary and populate the class cache
                            match &cursor_root_class {
                                None => {
                                    if let Ok(class) = real_window_class_w(cursor_root_hwnd) {
                                        class_cache.insert(cursor_root_hwnd, class.clone());
                                        cursor_root_class = Some(class);
                                    }
                                }
                                Some(class) => {
                                    tracing::debug!(
                                        "hwnd {cursor_root_hwnd} class was found in the cache: {class}"
                                    );
                                }
                            }

                            // make syscalls if necessary and populate the class cache
                            match &foreground_class {
                                None => {
                                    if let Ok(class) = real_window_class_w(foreground_hwnd) {
                                        class_cache.insert(foreground_hwnd, class.clone());
                                        foreground_class = Some(class);
                                    }
                                }
                                Some(class) => {
                                    tracing::debug!(
                                        "hwnd {foreground_hwnd} class was found in the cache: {class}"
                                    );
                                }
                            }

                            if let (Some(cursor_root_class), Some(foreground_class)) =
                                (&cursor_root_class, &foreground_class)
                            {
                                // steam fixes - populate the hwnd pair cache if necessary
                                if cursor_root_class == "Chrome_RenderWidgetHostHWND"
                                    && foreground_class == "SDL_app"
                                {
                                    hwnd_pair_cache.insert(cursor_root_hwnd, foreground_hwnd);
                                    continue;
                                }
                            }

                            // check our eligibility caches
                            if let (Some(cursor_root_is_eligible), Some(foreground_is_eligible)) = (
                                eligibility_cache.get(&cursor_root_hwnd),
                                eligibility_cache.get(&foreground_hwnd),
                            ) {
                                if *cursor_root_is_eligible && *foreground_is_eligible {
                                    should_raise = true;
                                    tracing::debug!(
                                        "hwnds {cursor_root_hwnd} and {foreground_hwnd} were found as eligible in the cache"
                                    );
                                }
                            } else if let Some(hwnds) = &hwnds {
                                // use the hwnds file if twm integration is enabled
                                if let Ok(raw_hwnds) = std::fs::read_to_string(hwnds) {
                                    let mut cursor_root_is_eligible = true;
                                    let mut foreground_is_eligible = true;

                                    // step one: test against the hwnds in the twm hwnds file
                                    cursor_root_is_eligible &=
                                        raw_hwnds.contains(&cursor_root_hwnd.to_string());
                                    foreground_is_eligible &=
                                        raw_hwnds.contains(&foreground_hwnd.to_string());

                                    // step two: test against known classes
                                    if let (Some(cursor_root_class), Some(foreground_class)) =
                                        (&cursor_root_class, &foreground_class)
                                    {
                                        for (class, strategy) in CLASS_IGNORELIST.iter() {
                                            let cursor_root_has_match =
                                                has_match(cursor_root_class, class, strategy);
                                            let foreground_has_match =
                                                has_match(foreground_class, class, strategy);

                                            cursor_root_is_eligible &= !cursor_root_has_match;
                                            foreground_is_eligible &= !foreground_has_match;
                                        }
                                    }

                                    // TODO: right now we just ignore the non-eligible case due to
                                    // potential delays with the twm writing to the hwnds file
                                    if cursor_root_is_eligible {
                                        eligibility_cache.insert(cursor_root_hwnd, true);
                                    }
                                    if foreground_is_eligible {
                                        eligibility_cache.insert(foreground_hwnd, true);
                                    }

                                    should_raise =
                                        cursor_root_is_eligible && foreground_is_eligible;
                                }
                            } else {
                                let mut cursor_root_is_eligible = true;
                                let mut foreground_is_eligible = true;

                                // step one: test against known window styles
                                cursor_root_is_eligible &= !has_filtered_style(cursor_root_hwnd);
                                foreground_is_eligible &= !has_filtered_style(foreground_hwnd);

                                // step two: test against known classes
                                if let (Some(cursor_root_class), Some(foreground_class)) =
                                    (&cursor_root_class, &foreground_class)
                                {
                                    for (class, strategy) in CLASS_IGNORELIST.iter() {
                                        let cursor_root_has_match =
                                            has_match(cursor_root_class, class, strategy);
                                        let foreground_has_match =
                                            has_match(foreground_class, class, strategy);

                                        cursor_root_is_eligible &= !cursor_root_has_match;
                                        foreground_is_eligible &= !foreground_has_match;
                                    }
                                }

                                eligibility_cache.insert(cursor_root_hwnd, cursor_root_is_eligible);
                                eligibility_cache.insert(foreground_hwnd, foreground_is_eligible);

                                should_raise = cursor_root_is_eligible && foreground_is_eligible;
                            }

                            if should_raise {
                                match raise_and_focus_window(cursor_root_hwnd) {
                                    Ok(_) => {
                                        tracing::info!("raised hwnd: {cursor_root_hwnd}");
                                    }
                                    Err(error) => {
                                        tracing::error!(
                                            "failed to raise hwnd {cursor_root_hwnd}: {error}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Event::MouseButton { action, .. } => match action {
                    Action::Press => is_mouse_down = true,
                    Action::Release => is_mouse_down = false,
                },
                _ => {}
            }
        }
    });
}

macro_rules! as_ptr {
    ($value:expr) => {
        $value as *mut core::ffi::c_void
    };
}

enum WindowsResult<T, E> {
    Err(E),
    Ok(T),
}

macro_rules! impl_from_integer_for_windows_result {
    ( $( $integer_type:ty ),+ ) => {
        $(
            impl From<$integer_type> for WindowsResult<$integer_type, color_eyre::eyre::Error> {
                fn from(return_value: $integer_type) -> Self {
                    match return_value {
                        0 => Self::Err(std::io::Error::last_os_error().into()),
                        _ => Self::Ok(return_value),
                    }
                }
            }
        )+
    };
}

impl_from_integer_for_windows_result!(usize, isize, u16, u32, i32);

impl<T, E> From<WindowsResult<T, E>> for Result<T, E> {
    fn from(result: WindowsResult<T, E>) -> Self {
        match result {
            WindowsResult::Err(error) => Err(error),
            WindowsResult::Ok(ok) => Ok(ok),
        }
    }
}

trait ProcessWindowsCrateResult<T> {
    fn process(self) -> Result<T>;
}

macro_rules! impl_process_windows_crate_integer_wrapper_result {
    ( $($input:ty => $deref:ty),+ $(,)? ) => (
        paste::paste! {
            $(
                impl ProcessWindowsCrateResult<$deref> for $input {
                    fn process(self) -> Result<$deref> {
                        if self == $input(std::ptr::null_mut()) {
                            Err(std::io::Error::last_os_error().into())
                        } else {
                            Ok(self.0 as $deref)
                        }
                    }
                }
            )+
        }
    );
}

impl_process_windows_crate_integer_wrapper_result!(
    HWND => isize,
);

impl<T> ProcessWindowsCrateResult<T> for WindowsCrateResult<T> {
    fn process(self) -> Result<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(error.into()),
        }
    }
}

fn has_match(str1: &str, str2: &str, matching_strategy: &MatchingStrategy) -> bool {
    match matching_strategy {
        MatchingStrategy::Equals => str1 == str2,
        MatchingStrategy::Contains => str1.contains(str2),
    }
}

fn get_window_ex_style(hwnd: isize) -> WINDOW_EX_STYLE {
    unsafe { WINDOW_EX_STYLE(GetWindowLongW(HWND(as_ptr!(hwnd)), GWL_EXSTYLE) as u32) }
}

fn has_filtered_style(hwnd: isize) -> bool {
    let ex_style = get_window_ex_style(hwnd);

    ex_style.contains(WS_EX_TOOLWINDOW) || ex_style.contains(WS_EX_NOACTIVATE)
}

fn get_ancestor(hwnd: isize, gaflags: GET_ANCESTOR_FLAGS) -> Result<isize> {
    unsafe { GetAncestor(HWND(as_ptr!(hwnd)), gaflags) }.process()
}

fn window_from_point(point: POINT) -> Result<isize> {
    unsafe { WindowFromPoint(point) }.process()
}

fn window_at_cursor_pos() -> Result<isize> {
    window_from_point(cursor_pos()?)
}

fn foreground_window() -> Result<isize> {
    unsafe { GetForegroundWindow() }.process()
}

fn cursor_pos() -> Result<POINT> {
    let mut cursor_pos = POINT::default();
    unsafe { GetCursorPos(&mut cursor_pos) }.process()?;

    Ok(cursor_pos)
}

fn raise_and_focus_window(hwnd: isize) -> Result<()> {
    let event = [INPUT {
        r#type: INPUT_MOUSE,
        ..Default::default()
    }];

    unsafe {
        // Send an input event to our own process first so that we pass the
        // foreground lock check
        SendInput(&event, size_of::<INPUT>() as i32);
        // Error ignored, as the operation is not always necessary.

        SetForegroundWindow(HWND(as_ptr!(hwnd)))
    }
    .ok()
    .process()
}

fn real_window_class_w(hwnd: isize) -> Result<String> {
    const BUF_SIZE: usize = 512;
    let mut class: [u16; BUF_SIZE] = [0; BUF_SIZE];

    let len = Result::from(WindowsResult::from(unsafe {
        RealGetWindowClassW(HWND(as_ptr!(hwnd)), &mut class)
    }))?;

    Ok(String::from_utf16(&class[0..len as usize])?)
}
