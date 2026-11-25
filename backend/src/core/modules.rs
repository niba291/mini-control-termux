use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModuleInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

pub fn registered_modules() -> Vec<ModuleInfo> {
    vec![
        ModuleInfo {
            id: "files",
            name: "File Manager",
            description: "File and storage management",
            enabled: true,
        },
        ModuleInfo {
            id: "services",
            name: "Service Manager",
            description: "Manage background services",
            enabled: true,
        },
        ModuleInfo {
            id: "system",
            name: "System Monitor",
            description: "System information and metrics",
            enabled: true,
        },
    ]
}
