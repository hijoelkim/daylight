use std::sync::Mutex;

#[derive(Clone, Copy)]
pub struct Browser {
    pub exe: &'static str,
    pub label: &'static str,
}

pub fn kind(exe_name: &str) -> Option<Browser> {
    match exe_name.to_ascii_lowercase().as_str() {
        "chrome.exe" => Some(Browser { exe: "chrome.exe", label: "Chrome" }),
        "msedge.exe" => Some(Browser { exe: "msedge.exe", label: "Edge" }),
        "firefox.exe" => Some(Browser { exe: "firefox.exe", label: "Firefox" }),
        _ => None,
    }
}

pub fn site_host(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("chrome:")
        || lower.starts_with("edge:")
        || lower.starts_with("about:")
        || lower.starts_with("file:")
        || lower.starts_with("devtools:")
        || lower.starts_with("view-source:")
    {
        return None;
    }
    let rest = lower.split_once("://").map(|(_, rest)| rest).unwrap_or(lower.as_str());
    let rest = rest.split(['/', '?', '#']).next().unwrap_or("");
    let rest = rest.rsplit('@').next().unwrap_or(rest);
    let host = rest.split(':').next().unwrap_or("").trim();
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() || !host.contains('.') || host.chars().any(|ch| ch.is_whitespace()) {
        return None;
    }
    Some(host.to_string())
}

pub fn private_title(title: Option<&str>) -> bool {
    let Some(title) = title else { return false };
    let lower = title.trim().to_ascii_lowercase();
    lower.ends_with("(incognito)")
        || lower.ends_with("(inprivate)")
        || lower.ends_with("(private browsing)")
}

struct Clock {
    browser: String,
    host: String,
    at: i64,
}

static CLOCK: Mutex<Option<Clock>> = Mutex::new(None);

pub fn take_delta(now: i64, browser: &str, host: &str) -> i64 {
    let mut guard = CLOCK.lock().unwrap_or_else(|err| err.into_inner());
    let delta = match guard.as_ref() {
        Some(prev) if prev.browser == browser && prev.host == host && prev.at > 0 => {
            (now - prev.at).clamp(0, 5_000)
        }
        _ => 1_000,
    };
    *guard = Some(Clock {
        browser: browser.to_string(),
        host: host.to_string(),
        at: now,
    });
    delta
}

#[cfg(windows)]
pub fn private_window(title: Option<&str>, hwnd: windows::Win32::Foundation::HWND) -> bool {
    private_title(title) || private_badge(hwnd)
}

#[cfg(windows)]
pub fn read_address(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationValuePattern, UIA_AutomationIdPropertyId, UIA_NamePropertyId,
        UIA_ValuePatternId,
    };

    unsafe {
        let auto: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let root = auto.ElementFromHandle(hwnd).ok()?;
        let named = find_by(&auto, &root, UIA_NamePropertyId, "Address and search bar");
        let by_id = named.or_else(|| find_by(&auto, &root, UIA_AutomationIdPropertyId, "urlbar-input"));
        let element = by_id?;
        let pattern: IUIAutomationValuePattern = element.GetCurrentPatternAs(UIA_ValuePatternId).ok()?;
        let value = pattern.CurrentValue().ok()?.to_string();
        if site_host(&value).is_some() {
            return Some(value);
        }
        let name = element.CurrentName().ok()?.to_string();
        site_host(&name).map(|_| name)
    }
}

#[cfg(windows)]
fn private_badge(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

    unsafe {
        let Ok(auto): Result<IUIAutomation, _> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) else {
            return false;
        };
        let Ok(root) = auto.ElementFromHandle(hwnd) else { return false };
        ["Incognito", "InPrivate", "Private Browsing"]
            .into_iter()
            .any(|name| button_named(&auto, &root, name))
    }
}

#[cfg(windows)]
fn button_named(
    auto: &windows::Win32::UI::Accessibility::IUIAutomation,
    root: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    name: &str,
) -> bool {
    use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_I4};
    use windows::Win32::UI::Accessibility::{
        TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_NamePropertyId,
    };

    unsafe {
        let Some(name_cond) = text_condition(auto, UIA_NamePropertyId, name) else { return false };
        let mut variant = VARIANT::default();
        let slot = std::ops::DerefMut::deref_mut(&mut variant.Anonymous.Anonymous);
        slot.vt = VT_I4;
        slot.Anonymous.lVal = UIA_ButtonControlTypeId.0;
        let type_cond = auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &variant).ok();
        let _ = VariantClear(&mut variant);
        let Some(type_cond) = type_cond else { return false };
        let Ok(both) = auto.CreateAndCondition(&name_cond, &type_cond) else { return false };
        root.FindFirst(TreeScope_Descendants, &both).is_ok()
    }
}

#[cfg(windows)]
fn text_condition(
    auto: &windows::Win32::UI::Accessibility::IUIAutomation,
    property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
    text: &str,
) -> Option<windows::Win32::UI::Accessibility::IUIAutomationCondition> {
    use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_BSTR};
    use windows::core::BSTR;

    unsafe {
        let mut variant = VARIANT::default();
        let slot = std::ops::DerefMut::deref_mut(&mut variant.Anonymous.Anonymous);
        slot.vt = VT_BSTR;
        slot.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(text));
        let condition = auto.CreatePropertyCondition(property, &variant).ok();
        let _ = VariantClear(&mut variant);
        condition
    }
}

#[cfg(windows)]
unsafe fn find_by(
    auto: &windows::Win32::UI::Accessibility::IUIAutomation,
    root: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    property: windows::Win32::UI::Accessibility::UIA_PROPERTY_ID,
    text: &str,
) -> Option<windows::Win32::UI::Accessibility::IUIAutomationElement> {
    use windows::Win32::UI::Accessibility::TreeScope_Descendants;

    let condition = text_condition(auto, property, text)?;
    root.FindFirst(TreeScope_Descendants, &condition).ok()
}
