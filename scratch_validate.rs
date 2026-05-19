use std::collections::HashMap;

fn main() {
    let mut agents = HashMap::<String, String>::new();
    let default = Some("phi001".to_string());
    
    // Add phi001 to agents
    agents.insert("other".to_string(), "val".to_string());
    
    let default_val = default.as_ref().unwrap();
    if !agents.is_empty() && !agents.contains_key(default_val) {
        println!("Error: Default agent '{}' not found in agents map", default_val);
    } else {
        println!("No error.");
    }
    
    // Now with empty agents
    agents.clear();
    if !agents.is_empty() && !agents.contains_key(default_val) {
        println!("Error with empty agents: Default agent '{}' not found in agents map", default_val);
    } else {
        println!("No error with empty agents.");
    }
}
