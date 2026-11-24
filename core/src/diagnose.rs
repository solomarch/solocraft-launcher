use once_cell::sync::Lazy;
use regex::Regex;

static RULES: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    let r = |p: &str, m: &'static str| (Regex::new(p).unwrap(), m);
    vec![
        r(
            r"UnsupportedClassVersionError|has been compiled by a more recent version of the Java Runtime",
            "This game or mod needs a newer Java than the one used. Set Java to Auto in the instance settings.",
        ),
        r(
            r"java\.lang\.OutOfMemoryError",
            "The game ran out of memory. Raise the maximum RAM in the instance settings.",
        ),
        r(
            r"Could not reserve enough space|Invalid maximum heap size|Could not create the Java Virtual Machine",
            "Java could not start with the chosen memory. Lower the maximum RAM or check the JVM arguments.",
        ),
        r(
            r"GLFW error 65542|does not appear to support OpenGL|Pixel format not accepted|GLFW error 65543",
            "Your graphics driver does not provide OpenGL to Java. Update the GPU driver, or try the NVIDIA GPU option in the instance settings.",
        ),
        r(
            r"UnsatisfiedLinkError",
            "A native library failed to load. Launch again to re-extract natives, or check that the Java architecture matches your system.",
        ),
        r(
            r"requires.*fabric-api|fabric-api.*requires|Mod 'Fabric API'",
            "A mod needs Fabric API. Install Fabric API in the instance.",
        ),
        r(
            r"Address already in use",
            "A network port is already in use. Close other game instances or servers and try again.",
        ),
    ]
});

static MIXIN_FAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Mixin \[[^\]]*? from mod ([A-Za-z0-9_.\-]+)\] from phase .*?FAILED|Mixin config [^\n]*? from mod ([A-Za-z0-9_.\-]+)").unwrap()
});

/// A mod whose mixin could not be applied is almost always built for another
/// Minecraft version. Name it so the user knows what to remove.
fn mixin_culprit(log: &str) -> Option<String> {
    let c = MIXIN_FAIL.captures(log)?;
    let id = c.get(1).or_else(|| c.get(2))?.as_str();
    Some(format!(
        "The mod '{id}' does not fit this Minecraft version (its code patch failed), so the game stopped while loading. Open Mods, remove or disable '{id}' (or update it), then launch again."
    ))
}

pub fn diagnose(log: &str) -> Option<String> {
    if let Some(m) = mixin_culprit(log) {
        return Some(m);
    }
    for (re, msg) in RULES.iter() {
        if re.is_match(log) {
            return Some((*msg).to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_known_crashes() {
        assert!(diagnose("Exception in thread main java.lang.OutOfMemoryError: Java heap space")
            .unwrap()
            .contains("memory"));
        assert!(diagnose("java.lang.UnsupportedClassVersionError: x").unwrap().contains("Java"));
        assert!(diagnose("all fine").is_none());
        let crash = "Caused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [cullleaves.mixins.json:MixinBlockModelRenderer from mod cullleaves] from phase [DEFAULT] in config [cullleaves.mixins.json] FAILED during APPLY";
        let m = diagnose(crash).unwrap();
        assert!(m.contains("'cullleaves'") && m.contains("remove"));
    }
}
