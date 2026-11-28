// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=app.manifest");
        let mut res = winres::WindowsResource::new();
        if std::path::Path::new("app.manifest").exists() {
            res.set_manifest_file("app.manifest");
        }
        if let Err(error) = res.compile() {
            println!("cargo:warning=manifest embedding failed: {error}");
        }
    }
}
