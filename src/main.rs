extern crate chrono;
extern crate regex;

mod term_verifier;

use term_verifier::CoqGen;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::process::{self, Command};
use std::str::{self, FromStr};

use chrono::DateTime;
use chrono::Utc;
use regex::Regex;

fn main() {
    // Input
    let args: Vec<String> = env::args().collect();
    let filename = args.get(1).expect("pass a filename as an argument");
    let inferred_name = Path::new(&filename).with_extension("");
    let inferred_name = inferred_name.file_name()
                                     .and_then(|n| n.to_str())
                                     .unwrap_or("signature");
    let mut f = File::open(filename).expect("the file could not be found");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("could not read the file");
    let lines = contents.split("\n");
    let re_nbop = Regex::new(r"^[a-z]+: \d+$").unwrap();
    let re_mv = Regex::new(r"^[A-Z][A-Z']*: \d+$").unwrap();
    let re_bop = Regex::new(r"^[a-z]+: \((\d+, )*\d+\)$").unwrap();
    let re_varset = Regex::new(r"^V := \{((([a-z]+), )*([a-z]+)?)\}$").unwrap();
    let re_sig = Regex::new(r"^\(((\d+, )*\d+)\)$").unwrap();
    let re_comment = Regex::new(r"^#.*$").unwrap();
    let mut gen_vars: Option<Vec<String>> = None;
    let mut metavars: HashMap<&str, u8> = HashMap::new();
    let ops: Vec<_> = lines.enumerate().filter_map(|(i, s)| {
        let s = s.trim();
        if s.is_empty() || re_comment.is_match(s) {
            return None;
        }
        if re_bop.is_match(s) {
            let comps: Vec<_> = s.split(": ").into_iter().collect();
            let (name, sig) = (comps[0], comps[1]);
            let sig: Vec<_> = re_sig.replace(sig, "$1")
                                     .split(", ")
                                     .map(|x| u8::from_str(x).unwrap())
                                     .collect();
            Some((name, sig))
        } else if re_nbop.is_match(s) {
            let comps: Vec<_> = s.split(": ").into_iter().collect();
            let (name, sig) = (comps[0], comps[1]);
            Some((name, vec![0; usize::from_str(sig).unwrap()]))
        } else if re_mv.is_match(s) {
            let comps: Vec<_> = s.split(": ").into_iter().collect();
            let (name, arity) = (comps[0], comps[1]);
            if metavars.contains_key(name) {
                panic!("metavariable provided multiple times");
            }
            metavars.insert(name, u8::from_str(arity).unwrap());
            None
        } else if re_varset.is_match(s) {
            // TODO: make sure gen_vars doesn't exceed u8 limit
            if gen_vars.is_none() {
                let capt = re_varset.captures_iter(s).next().unwrap()[1].to_string();
                let vars: Vec<String> = capt.split(", ").filter_map(|v| {
                    if !v.is_empty() {
                        Some(format!(r#""{}""#, v))
                    } else {
                        None
                    }
                }).collect();
                gen_vars = Some(vars);
                None
            } else {
                panic!("declared V twice");
            }
        } else {
            panic!("incorrect signature format on line {}: {}", i + 1, s);
        }
    }).collect();
    if ops.len() > 9 {
        panic!("binding signatures may only contain up to 9 operators")
    }
    // TODO: Check validity of names

    // Output
    let mut f = File::open("src/term_verifier.rs")
                     .expect("the term verifier template file could not be found");
    let mut template = String::new();
    f.read_to_string(&mut template).expect("could not read the term verifier template file");

    let re_ignore_on = Regex::new(r"^\s*/\*\s*\[\[\s*IGNORE\s*\*/\s*$").unwrap();
    let re_ignore_off = Regex::new(r"^\s*/\*\s*IGNORE\s*\]\]\s*\*/\s*$").unwrap();
    let re_cmd =
        Regex::new(r"^(\s*).*?/\*\s*\[\[\s*(INSERT:\s*(\w+)|IGNORE)\s*\]\]\s*\*/\s*$").unwrap();
    let mut ignore_on = false;
    let gen_vars = gen_vars.unwrap_or_else(|| vec![]);
    let metavars = metavars.into_iter()
                           .map(|(name, arity)| {
                               format!("({:?}, vec![{}]),",
                                       name,
                                       vec!["0"; arity as usize].join(", "))
                           })
                           .collect::<Vec<_>>();
    let output = template.split("\n").filter_map(|line| {
        let matches: Vec<_> = re_cmd.captures_iter(line).into_iter().collect();
        if !matches.is_empty() {
            let matched = &matches[0];
            let indentation = matched.get(1).unwrap().as_str();
            let cmd = matched.get(2).unwrap().as_str();
            let processed: Option<String> = match cmd {
                "IGNORE" => {
                    None
                }
                _ => {
                    // Must be "INSERT"
                    match matched.get(3).unwrap().as_str() {
                        "header" => {
                            let now: DateTime<Utc> = Utc::now();
                            Some(format!("/* Autogenerated Rust file: {} */",
                                         now.format("%Y-%m-%d %H:%M:%S")))
                        }
                        "inferred_name" => {
                            Some(format!(r#"inferred_name = "{}";"#, inferred_name))
                        }
                        "ops" => {
                            let ops_string = ops.iter().map(|&(name, ref sig)| {
                                format!(r#"("{}", vec!{:?}),"#, name, sig)
                            }).collect::<Vec<_>>().join(&format!("\n{}", indentation));
                            Some(ops_string)
                        }
                        "gen_vars" => {
                            Some(gen_vars.join(", "))
                        }
                        "metavars" => {
                            Some(metavars.join(&format!("\n{}", indentation)))
                        }
                        tag => panic!("unrecognised template INSERT tag: {}", tag)
                    }
                }
            };
            return processed.map(|s| format!("{}{}", indentation, s));
        }
        if re_ignore_on.is_match(line) {
            ignore_on = true;
        }
        let keep_line = if ignore_on {
            None
        } else {
            Some(line.to_string())
        };
        if re_ignore_off.is_match(line) {
            ignore_on = false;
        }
        keep_line
    }).collect::<Vec<_>>().join("\n");

    // Generate the term verification tool.
    let cd = env::current_dir()
                 .expect("couldn't get the current directory");
    let mut source_out_dir = cd.to_path_buf();
    source_out_dir.push(&format!("out/{}-term-verifier.rs", inferred_name));
    let mut f = File::create(source_out_dir.clone())
                     .expect("could not create a term verifier file");
    write!(f, "{}", output).expect("could not create a term verifier");
    println!("Generated a new file at: {}", source_out_dir.to_string_lossy());

    let mut deps = cd.clone();
    deps.push("target/debug/deps");
    let deps = deps.to_string_lossy();

    let out_dir = source_out_dir.parent().expect("couldn't find the out/ directory");

    // Compile the term verification tool.
    // TODO: Fix the hard-coded crate hashes.
    let output = Command::new("rustc")
                         .args(&[
                            &source_out_dir.to_string_lossy(),
                            "--out-dir",
                            &out_dir.to_string_lossy(),
                            "-L",
                            &format!("dependency={}", deps),
                            "--extern",
                            &format!("chrono={}/libchrono-a31d51bb8f8b101c.rlib", deps),
                            "--extern",
                            &format!("regex={}/libregex-44aa0d49f4a94993.rlib", deps),
                         ])
                         .output()
                         .expect("could not compile the term verifier");
    if output.status.success() {
        println!("Compiled the program at: {}",
            source_out_dir.with_extension("").to_string_lossy());
    } else {
        println!("Issue encountered while compiling the term verifier:\n{}\n{}",
                 str::from_utf8(&output.stdout).expect("could not convert stdout to a string"),
                 str::from_utf8(&output.stderr).expect("could not convert stderr to a string"));
        process::exit(1);
    }

    let coq_gen = CoqGen { name: inferred_name, ops: &ops, metavars: &vec![] };

    // Output
    let output = format!("{}\n", coq_gen.gen_colimit());
    if false {
        println!();
        println!("{}", output);
    }
    // Substitution initial algebra file
    let mut f = File::create(Path::new(&format!("out/{}-substitution-algebra", coq_gen.name))
                     .with_extension("v"))
                     .expect("could not create the generated Coq substitution algebra file");
    write!(f, "{}", output).expect("could not write to the generated Coq substitution algebra file");
    println!("Generated a construction of the substitution algebra at: {}-substitution-algebra.v",
             coq_gen.name);
}
