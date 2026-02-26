import sys

with open('src/fs.rs', 'r') as f:
    fs_content = f.read()

old_fs = """        if let Some(parent) = dst_abs.parent() {
            if let Ok(parent_canon) = fs::canonicalize(parent) {
                if parent_canon.starts_with(&src_canon) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Cannot copy directory into itself: {:?} is inside {:?}", dst_abs, src_canon),
                    ).into());
                }
            }
        }"""

new_fs = """        if let Some(parent_canon) = dst_abs.parent().and_then(|p| fs::canonicalize(p).ok()) {
            if parent_canon.starts_with(&src_canon) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Cannot copy directory into itself: {:?} is inside {:?}", dst_abs, src_canon),
                ).into());
            }
        }"""

fs_content = fs_content.replace(old_fs, new_fs)
with open('src/fs.rs', 'w') as f:
    f.write(fs_content)

with open('src/mcp.rs', 'r') as f:
    mcp_content = f.read()

old_mcp1 = """        if let Some(key) = self.wrapper_key {
            if let Some(wrapped) = parsed.get(key) {
                if let Some(obj) = wrapped.as_mapping() {
                   let mut res = HashMap::new();
                   for (k, v) in obj {
                       let key_str = k.as_str().unwrap_or_default().to_string();
                       let json_v = serde_json::to_value(v)?;
                       res.insert(key_str, json_v);
                   }
                   return Ok(res);
                }
            }
        }"""

new_mcp1 = """        if let Some(obj) = self.wrapper_key.and_then(|key| parsed.get(key)).and_then(|v| v.as_mapping()) {
            let mut res = HashMap::new();
            for (k, v) in obj {
                let key_str = k.as_str().unwrap_or_default().to_string();
                let json_v = serde_json::to_value(v)?;
                res.insert(key_str, json_v);
            }
            return Ok(res);
        }"""

old_mcp2 = """        if let Some(mcp) = parsed.get("mcpServers") {
            if let Some(obj) = mcp.as_object() {
                return Ok(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
            }
        }"""

new_mcp2 = """        if let Some(obj) = parsed.get("mcpServers").and_then(|mcp| mcp.as_object()) {
            return Ok(obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        }"""

mcp_content = mcp_content.replace(old_mcp1, new_mcp1).replace(old_mcp2, new_mcp2)
with open('src/mcp.rs', 'w') as f:
    f.write(mcp_content)
