use takumi::layout::style::tw::TailwindProperty;

fn main() {
    // Test parsing of arbitrary color
    match TailwindProperty::parse("text-[deepskyblue]") {
        Some(TailwindProperty::Color(color_input)) => {
            println!("Successfully parsed text-[deepskyblue] to: {:?}", color_input);
        }
        Some(other) => {
            println!("Parsed to unexpected type: {:?}", other);
        }
        None => {
            println!("Failed to parse text-[deepskyblue]");
        }
    }
    
    // Test parsing of predefined color still works
    match TailwindProperty::parse("text-red-500") {
        Some(TailwindProperty::Color(color_input)) => {
            println!("Successfully parsed text-red-500 to: {:?}", color_input);
        }
        Some(other) => {
            println!("Parsed to unexpected type: {:?}", other);
        }
        None => {
            println!("Failed to parse text-red-500");
        }
    }
}
