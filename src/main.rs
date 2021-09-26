use anyhow;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use itertools::Itertools; // for join on hashset
use std::io;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs::File;

type Username = String;
type GhUsername = String;

#[derive(Debug, Deserialize)]
struct StudentFeedback {
    email_addr: String,
    school_username: Username,
    github_username: GhUsername,
    last_teammate_github_username: Option<GhUsername>,
    last_teammate_feedback: Option<String>,
    last_teammate_additional_feedback: Option<String>,
    veto0: Option<Username>,
    veto1: Option<Username>,
    veto2: Option<Username>,
    want0: Option<Username>,
    want1: Option<Username>,
    want2: Option<Username>,
}

#[derive(Debug, Deserialize)]
struct StudentClassification {
    school_username: Username,
    classifications: Option<String>
}

#[derive(Debug, Deserialize)]
struct ClassificationRelations {
    class: String,
    attraction: Option<String>,
    detraction: Option<String>
}

fn parse() -> anyhow::Result<(Vec<StudentFeedback>, Vec<StudentClassification>, Vec<ClassificationRelations>)> {
    let file_path = env::args_os().nth(1)?;
    let feedback_path = File::open(file_path)?;
    let file_path = env::args_os().nth(2)?;
    let classification_path = File::open(file_path)?;
    let file_path = env::args_os().nth(3)?;
    let classification_relations_path = File::open(file_path)?;

    let mut fb_rdr = csv::Reader::from_reader(feedback_path);
    let mut sc_rdr = csv::Reader::from_reader(classification_path);
    let mut rel_rdr = csv::Reader::from_reader(classification_relations_path);

    let mut fb = Vec::new();
    let mut sc = Vec::new();
    let mut rel = Vec::new();

    for result in fb_rdr.records() {
        let fb_record: StudentFeedback = result?;

	fb.push(fb_record);
    }

    for result in sc_rdr.records() {
        let c_record: StudentClassification = result?;

	sc.push(c_record);
    }

    for result in rel_rdr.records() {
        let c_record: ClassificationRelations = result?;

	rel.push(record);
    }

    Ok((fb, sc, rel))
}

struct Student {
    name: String,
    school_username: Username,
    github_username: GhUsername,
    attractors: HashSet<Username>,
    repulsors: HashSet<Username>,
}

fn student_matrix(fb: StudentFeedback, sc: StudentClassification, cr: ClassificationRelations) {

}

fn main() -> anyhow::Result<()> {
    let mut feedback_rdr = csv::Reader::from_reader(io::stdin());
    let mut classification_rdr = csv::Reader::from_reader(io::stdin());

    let (fb, c) = parse()?;
}
