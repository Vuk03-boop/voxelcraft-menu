use crate::config::Config;
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade { None, Warm, Cine }
impl Grade {
    pub fn index(self) -> u32 { match self { Self::None => 0, Self::Warm => 1, Self::Cine => 2 } }
    pub fn label(self) -> &'static str { match self { Self::None => "Natural", Self::Warm => "Warm", Self::Cine => "Cinema" } }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub scale: f32,
    pub fov: f32,
    pub vsync: bool,
    pub taa: bool,
    pub shadows: bool,
    pub ao: bool,
    pub reflections: bool,
    pub refraction: bool,
    pub aces: bool,
    pub grade: Grade,
    pub grade_strength: f32,
    pub fog_density: f32,
    pub ambient: f32,
    pub compact_shade_hit: bool,
    pub isolate_glass: bool,
    pub shadow_pass: bool,
    pub glass_reflect: bool,
    pub water_look: bool,
    pub wind_sway: bool,
    pub soft_shadows: bool,

}
impl Default for Preferences {
    fn default() -> Self { Self::from_config(&Config::default()) }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplyPlan {
    pub resize: bool,
    pub surface: bool,
    pub presentation: bool,
    pub history: bool,
    pub experiments: bool,
}

impl Preferences {
    pub fn from_config(c: &Config) -> Self {
        Self { scale: c.render_scale, fov: c.fov_degrees, vsync: c.vsync, taa: c.taa,
            shadows: c.shadows, ao: c.ao, reflections: c.water_reflect, refraction: c.water_refract,
            aces: c.tone_map_aces, grade: if c.grade_cine { Grade::Cine } else if c.grade_warm { Grade::Warm } else { Grade::None },
            grade_strength: c.grade_strength, fog_density: c.fog.density, ambient: c.ambient, compact_shade_hit: c.compact_shade_hit, isolate_glass: c.isolate_glass, shadow_pass: true, glass_reflect: c.glass_reflect, water_look: c.water_look, wind_sway: c.wind_sway, soft_shadows: c.soft_shadows }
    }

    pub fn keep_experiments_from(&mut self, other: &Self) {
        self.compact_shade_hit = other.compact_shade_hit;
        self.isolate_glass = other.isolate_glass;
        self.shadow_pass = other.shadow_pass;
        self.glass_reflect = other.glass_reflect;
        self.water_look = other.water_look;
        self.wind_sway = other.wind_sway;
        self.soft_shadows = other.soft_shadows;
    }
    pub fn experiments_differ(&self, other: &Self) -> bool {
        self.isolate_glass != other.isolate_glass || self.shadow_pass != other.shadow_pass || self.compact_shade_hit != other.compact_shade_hit || self.glass_reflect != other.glass_reflect || self.water_look != other.water_look || self.wind_sway != other.wind_sway || self.soft_shadows != other.soft_shadows
    }
    pub fn validate(&self) -> Result<(), String> {
        for (label, v, lo, hi) in [
            ("Render scale", self.scale, 0.25, 1.0), ("Field of view", self.fov, 30.0, 120.0),
            ("Grade strength", self.grade_strength, 0.0, 1.0), ("Fog density", self.fog_density, 0.0, 0.01),
            ("Ambient light", self.ambient, 0.0, 0.5),
        ] {
            if !v.is_finite() || v < lo || v > hi { return Err(format!("{label} must be finite and in {lo}..{hi}")); }
        }
        Ok(())
    }
    pub fn write_config(&self, c: &mut Config) {
        c.render_scale = self.scale; c.fov_degrees = self.fov; c.vsync = self.vsync; c.taa = self.taa;
        c.shadows = self.shadows; c.ao = self.ao; c.water_reflect = self.reflections; c.water_refract = self.refraction;
        c.tone_map_aces = self.aces; c.grade_warm = self.grade == Grade::Warm; c.grade_cine = self.grade == Grade::Cine;
        c.grade_strength = self.grade_strength; c.fog.density = self.fog_density; c.ambient = self.ambient;
        c.compact_shade_hit = self.compact_shade_hit;
        c.isolate_glass = self.isolate_glass;
        c.shadow_pass = true;
        c.glass_reflect = self.glass_reflect;
        c.water_look = self.water_look;
        c.wind_sway = self.wind_sway;
        c.soft_shadows = self.soft_shadows;

    }
    pub fn plan(&self, next: &Self) -> ApplyPlan {
        let resize = self.scale != next.scale;
        ApplyPlan { resize, experiments: self.experiments_differ(next), surface: self.vsync != next.vsync,
            presentation: self.aces != next.aces || self.grade != next.grade || self.grade_strength != next.grade_strength,
            history: self.experiments_differ(next) || resize || self.fov != next.fov || self.taa != next.taa || self.shadows != next.shadows
                || self.ao != next.ao || self.reflections != next.reflections || self.refraction != next.refraction
                || self.fog_density != next.fog_density || self.ambient != next.ambient }
    }

    pub fn overlay_cli(&mut self, parsed: &Config, args: &[String]) {
        let c = Self::from_config(parsed);
        for arg in args {
            match arg.as_str() {
                "--scale" => self.scale = c.scale, "--fov" => self.fov = c.fov,
                "--no-vsync" => self.vsync = c.vsync, "--no-taa" => self.taa = c.taa,
                "--no-shadows" => self.shadows = c.shadows, "--no-ao" => self.ao = c.ao,
                "--no-water-reflect" => self.reflections = c.reflections,
                "--no-water-refract" => self.refraction = c.refraction,
                "--tone-map" => self.aces = c.aces, "--grade" => self.grade = c.grade,
                "--grade-strength" => self.grade_strength = c.grade_strength,
                "--fog-density" => self.fog_density = c.fog_density, "--ambient" => self.ambient = c.ambient,
                "--isolate-glass" | "--no-isolate-glass" => self.isolate_glass = c.isolate_glass,
                "--shadow-pass" | "--no-shadow-pass" => self.shadow_pass = c.shadow_pass,
                "--compact-shade-hit" | "--no-compact-shade-hit" => self.compact_shade_hit = c.compact_shade_hit,
                "--glass-reflect" | "--no-glass-reflect" => self.glass_reflect = c.glass_reflect,
                "--water-look" | "--no-water-look" => self.water_look = c.water_look,
                "--wind-sway" | "--no-wind-sway" => self.wind_sway = c.wind_sway,
                "--soft-shadows" | "--no-soft-shadows" => self.soft_shadows = c.soft_shadows,
                _ => {}
            }
        }
    }
    pub fn encode(&self) -> String {
        format!("version=3\nscale={}\nfov={}\nvsync={}\ntaa={}\nshadows={}\nao={}\nreflections={}\nrefraction={}\naces={}\ngrade={}\ngrade_strength={}\nfog_density={}\nambient={}\ncompact_shade_hit={}\nglass_reflect={}\nwater_look={}\nwind_sway={}\nsoft_shadows={}\nisolate_glass={}\nshadow_pass={}\n",
            self.scale,self.fov,self.vsync,self.taa,self.shadows,self.ao,self.reflections,self.refraction,
            self.aces,self.grade.index(),self.grade_strength,self.fog_density,self.ambient,self.compact_shade_hit,self.glass_reflect,self.water_look,self.wind_sway,self.soft_shadows,self.isolate_glass,self.shadow_pass)
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut p = Self::default(); let mut seen = BTreeSet::new(); let mut version = 0;
        macro_rules! value { ($v:expr) => { $v.parse().map_err(|_| format!("Invalid preference value: {}", $v))? }; }
        for line in text.lines().map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')) {
            let (k,v) = line.split_once('=').ok_or("Expected preference key=value")?;
            let (k,v) = (k.trim(),v.trim());
            if !seen.insert(k) { return Err(format!("Duplicate preference: {k}")); }
            match k {
                "version" => { if v != "1" && v != "2" && v != "3" { return Err("Unsupported settings version; original file kept".into()); } version = v.parse::<u32>().map_err(|e| e.to_string())?; }
                "scale" => p.scale = value!(v), "fov" => p.fov = value!(v), "vsync" => p.vsync = value!(v),
                "taa" => p.taa = value!(v), "shadows" => p.shadows = value!(v), "ao" => p.ao = value!(v),
                "reflections" => p.reflections = value!(v), "refraction" => p.refraction = value!(v),
                "aces" => p.aces = value!(v), "grade_strength" => p.grade_strength = value!(v),
                "fog_density" => p.fog_density = value!(v), "ambient" => p.ambient = value!(v),
                "isolate_glass" => p.isolate_glass = value!(v),
                "shadow_pass" => p.shadow_pass = value!(v),
                "compact_shade_hit" => p.compact_shade_hit = value!(v),
                "glass_reflect" => p.glass_reflect = value!(v),
                "water_look" => p.water_look = value!(v),
                "wind_sway" => p.wind_sway = value!(v),
                "soft_shadows" => p.soft_shadows = value!(v),
                "grade" => p.grade = match v { "0" => Grade::None, "1" => Grade::Warm, "2" => Grade::Cine, _ => return Err("Unknown grade".into()) },
                _ => return Err(format!("Unknown preference {k}; original file kept")),
            }
        }
        if version == 0 { return Err("Missing settings version".into()); }
        if version == 1 && ["compact_shade_hit", "glass_reflect", "water_look", "wind_sway", "soft_shadows"].iter().any(|k| seen.contains(k)) {
            return Err("Experimental preference keys require version=2".into());
        }
        if version < 3 && ["isolate_glass", "shadow_pass"].iter().any(|k| seen.contains(k)) {
            return Err("Architecture preference keys require version=3".into());
        }
        p.shadow_pass=true;
        p.validate()?; Ok(p)
    }
}

pub fn preference_path() -> PathBuf {
    if let Some(p) = std::env::var_os("VOXELCRAFT_SETTINGS") { return PathBuf::from(p); }
    #[cfg(target_os = "windows")]
    if let Some(p) = std::env::var_os("APPDATA") { return PathBuf::from(p).join("voxelcraft/settings.conf"); }
    if let Some(p) = std::env::var_os("XDG_CONFIG_HOME") { return PathBuf::from(p).join("voxelcraft/settings.conf"); }
    if let Some(p) = std::env::var_os("HOME") { return PathBuf::from(p).join(".config/voxelcraft/settings.conf"); }
    PathBuf::from(".voxelcraft-settings.conf")
}
pub fn load(path: &Path) -> Result<Option<Preferences>, String> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(m) if m.len() > 65536 => return Err("Settings file too large".into()),
        Ok(_) => {}
    }

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut text = String::new();
    file.take(65537).read_to_string(&mut text).map_err(|e| e.to_string())?;
    if text.len() > 65536 { return Err("Settings file too large".into()); }
    Preferences::decode(&text).map(Some)
}

pub fn save(path: &Path, prefs: &Preferences) -> Result<(), String> {
    prefs.validate()?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let name = path.file_name().ok_or("Settings path needs a filename")?.to_string_lossy();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let temp = parent.join(format!(".{name}.{}.{nonce}.tmp",std::process::id()));
    let mut file = std::fs::OpenOptions::new().create_new(true).write(true).open(&temp).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(prefs.encode().as_bytes())?; file.sync_all()?; drop(file);
        std::fs::rename(&temp,path)
    })();
    if let Err(e) = result { let _ = std::fs::remove_file(&temp); return Err(e.to_string()); }
    Ok(())
}
