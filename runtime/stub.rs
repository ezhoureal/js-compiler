use std::collections::HashSet;

#[repr(C)]
#[derive(PartialEq, Eq, Copy, Clone)]
struct SnakeVal(u64);

#[repr(C)]
struct SnakeArray {
    size: u64,
    elts: *const SnakeVal,
}

/* You can use this function to cast a pointer to an array on the heap
 * into something more convenient to access
 *
 */
fn load_snake_array(address: u64) -> SnakeArray {
    unsafe {
        let p: *const u64 = std::mem::transmute(address - 1);
        let size = *p;
        SnakeArray {
            size,
            elts: std::mem::transmute(p.add(1)),
        }
    }
}

static TAG_MASK: u64 = 0x00_00_00_00_00_00_00_01;
static SNAKE_TRU: SnakeVal = SnakeVal(0xFF_FF_FF_FF_FF_FF_FF_FF);
static SNAKE_FLS: SnakeVal = SnakeVal(0x7F_FF_FF_FF_FF_FF_FF_FF);

// reinterprets the bytes of an unsigned number to a signed number
fn unsigned_to_signed(x: u64) -> i64 {
    i64::from_le_bytes(x.to_le_bytes())
}

fn print_array(a: &SnakeArray, visited: &mut HashSet<u64>) -> String {
    let mut s = "[".to_string();
    let mut p = a.elts;
    for i in 0..a.size {
        unsafe {
            s += &sprint_snake_val_inner(*p, visited);
            p = p.add(1);
        }
        if i < a.size - 1 {
            s += ", ";
        } else {
            s += "]";
        }
    }
    s.to_string()
}

fn sprint_snake_val_inner(x: SnakeVal, visited: &mut HashSet<u64>) -> String {
    if x.0 & TAG_MASK == 0 {
        // it's a number
        format!("{}", unsigned_to_signed(x.0) >> 1)
    } else if x == SNAKE_TRU {
        String::from("true")
    } else if x == SNAKE_FLS {
        String::from("false")
    } else if x.0 & 0b111 == 1 {
        if visited.contains(&x.0) {
            return "<loop>".to_string();
        }
        visited.insert(x.0);
        let array = load_snake_array(x.0);
        print_array(&array, visited)
    } else if x.0 & 0b111 == 0b11 {
        "<closure>".to_string()
    } else {
        format!("Invalid snake value 0x{:x}", x.0)
    }
}

fn sprint_snake_val(x: SnakeVal) -> String {
    sprint_snake_val_inner(x, &mut HashSet::new())
}

#[export_name = "\x01print_snake_val"]
extern "sysv64" fn print_snake_val(v: SnakeVal) -> SnakeVal {
    println!("{}", sprint_snake_val(v));
    return v;
}

/* Implement the following error function. You are free to change the
 * input and output types as needed for your design.
 *
**/
type ErrorCode = u64;
static ARITH_TYPE_ERROR: ErrorCode = 0;
static CMP_TYPE_ERROR: ErrorCode = 1;
static OVERFLOW_ERROR: ErrorCode = 2;
static IF_TYPE_ERROR: ErrorCode = 3;
static LOGIC_TYPE_ERROR: ErrorCode = 4;
static NON_ARRAY_ERROR: ErrorCode = 5;
static INDEX_NOT_NUMBER: ErrorCode = 6;
static INDEX_OUT_OF_BOUNDS: ErrorCode = 7;
static NON_CLOSURE_ERROR: ErrorCode = 8;
static LAMBDA_ARITY_ERROR: ErrorCode = 9;

#[export_name = "\x01snake_error"]
extern "sysv64" fn snake_error(err_code: ErrorCode, v: SnakeVal) {
    if err_code == ARITH_TYPE_ERROR {
        eprintln!("arithmetic expected a number {}", sprint_snake_val(v));
    } else if err_code == CMP_TYPE_ERROR {
        eprintln!("comparison expected a number {}", sprint_snake_val(v));
    } else if err_code == OVERFLOW_ERROR {
        eprintln!("overflow {}", sprint_snake_val(v));
    } else if err_code == IF_TYPE_ERROR {
        eprintln!("if expected a boolean {}", sprint_snake_val(v));
    } else if err_code == LOGIC_TYPE_ERROR {
        eprintln!("logic expected a boolean {}", sprint_snake_val(v));
    } else if err_code == NON_ARRAY_ERROR {
        eprintln!("not an array address {}", sprint_snake_val(v));
    } else if err_code == INDEX_NOT_NUMBER {
        eprintln!("index not a number: {}", sprint_snake_val(v));
    } else if err_code == INDEX_OUT_OF_BOUNDS {
        eprintln!("index out of bounds: {}", sprint_snake_val(v));
    } else if err_code == NON_CLOSURE_ERROR {
        eprintln!("called a non-function {}", sprint_snake_val(v));
    } else if err_code == LAMBDA_ARITY_ERROR {
        eprintln!("wrong number of arguments for lambda: {}", sprint_snake_val(v));
    } else if err_code == 99 {
        eprintln!("stack error: {:x}", v.0);
    } else {
        eprintln!("Unknown error {}", err_code);
    }
    std::process::exit(1);
}

#[link(name = "compiled_code", kind = "static")]
extern "C" {
    #[link_name = "\x01start_here"]
    fn start_here() -> SnakeVal;
}

fn main() {
    let output = unsafe { start_here() };
    let _ = print_snake_val(output);
}
