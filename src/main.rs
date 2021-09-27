use anyhow;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;

use rand::seq::SliceRandom;
use rand::thread_rng;

type Username = String;
type GhUsername = String;

#[derive(Debug, Deserialize)]
struct StudentFeedback {
    email_addr: String,
    school_username: Username,
    github_username: GhUsername,
    solo: String,
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
    name: String,
    school_username: Username,
    github_username: GhUsername,
    classifications: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClassificationRelations {
    class: String,
    class_other: String,
    relation: String, // either "+" or "-"
}

fn parse() -> anyhow::Result<(
    Vec<StudentFeedback>,
    Vec<StudentClassification>,
    Vec<ClassificationRelations>,
)> {
    let file_path = env::args_os()
        .nth(1)
        .ok_or(anyhow::anyhow!("First argument not provided."))?;
    let feedback_path = File::open(file_path)?;
    let file_path = env::args_os()
        .nth(2)
        .ok_or(anyhow::anyhow!("Second argument not provided."))?;
    let classification_path = File::open(file_path)?;
    let file_path = env::args_os()
        .nth(3)
        .ok_or(anyhow::anyhow!("Third argument not provided."))?;
    let classification_relations_path = File::open(file_path)?;

    let mut fb_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .comment(Some(b'#'))
	.flexible(true)
        .from_reader(feedback_path);
    let mut sc_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .comment(Some(b'#'))
	.flexible(true)
        .from_reader(classification_path);
    let mut rel_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .comment(Some(b'#'))
	.flexible(true)
        .from_reader(classification_relations_path);

    let mut fb = Vec::new();
    let mut sc = Vec::new();
    let mut rel = Vec::new();

    for result in fb_rdr.deserialize() {
        let mut fb_record: StudentFeedback = result?;

        fb_record.school_username = fb_record.school_username.trim().to_ascii_lowercase();
        fb.push(fb_record);
    }

    for result in sc_rdr.deserialize() {
        let c_record: StudentClassification = result?;

        sc.push(c_record);
    }

    for result in rel_rdr.deserialize() {
        let c_record: ClassificationRelations = result?;

        rel.push(c_record);
    }

    Ok((fb, sc, rel))
}

#[derive(Debug)]
struct Student {
    _email: String,
    school_username: Username,
    _github_username: GhUsername,
    attractors: HashSet<Username>,
    repulsors: HashSet<Username>,
    wants: Vec<Username>,
    vetos: Vec<Username>,
}

impl Student {
    fn new(email: String, school_username: Username, github_username: GhUsername) -> Self {
        Student {
            _email: email,
            school_username,
            _github_username: github_username,
            attractors: HashSet::new(),
            repulsors: HashSet::new(),
            vetos: Vec::new(),
            wants: Vec::new(),
        }
    }
}

type Students = HashMap<Username, Student>;

fn student_matrix(
    fb: Vec<StudentFeedback>,
    sc: Vec<StudentClassification>,
    cr: Vec<ClassificationRelations>,
) -> Students {
    let mut students = HashMap::new();
    let mut classification_students: HashMap<String, HashSet<Username>> = HashMap::new();
    let mut student_classification: HashMap<Username, HashSet<String>> = HashMap::new();
    let mut attractor_class: HashMap<String, HashSet<Username>> = HashMap::new();
    let mut repulsor_class: HashMap<String, HashSet<Username>> = HashMap::new();

    for s in &sc {
        // multiple instances of the student in the student classification spreadsheet?
        assert_eq!(students.contains_key(&s.school_username), false);
        students.insert(
            s.school_username.clone(),
            Student::new(
                s.name.clone(),
                s.school_username.clone(),
                s.github_username.clone(),
            ),
        );
        student_classification.insert(s.school_username.clone(), HashSet::new());

        // populate the classifications, and add students to the classifications
        for c in s
            .classifications
            .as_ref()
            .unwrap_or(&String::from(""))
            .split(",")
            .map(|s| String::from(s))
        {
            if c == "" {
                break;
            }

            if !classification_students.contains_key(&c.to_string()) {
                classification_students.insert(c.to_string(), HashSet::new());
            }
            // unwrap valid given the insert above
            classification_students
                .get_mut(&c.clone())
                .unwrap()
                .insert(s.school_username.clone());

            student_classification
                .get_mut(&s.school_username)
                .unwrap()
                .insert(c.clone());
        }
    }

    // Add some order to the classes, and which attract, and which
    // repulse.
    for c in &cr {
        if !attractor_class.contains_key(&c.class) {
            attractor_class.insert(c.class.clone(), HashSet::new());
        }
        if !repulsor_class.contains_key(&c.class) {
            repulsor_class.insert(c.class.clone(), HashSet::new());
        }
        if c.relation.contains("+") {
            let att = attractor_class.get_mut(&c.class).unwrap();

            if let Some(shs) = classification_students.get(&c.class_other) {
                for s in shs.iter() {
                    att.insert(s.clone());
                }
            }
        }
        if c.relation.contains("-") {
            let rep = repulsor_class.get_mut(&c.class).unwrap();

            if let Some(shs) = classification_students.get(&c.class_other) {
                for s in shs.iter() {
                    rep.insert(s.clone());
                }
            }
        }
    }

    // Lets take the student's feedback into account. For now,
    // consider the vetoed and wanted teammates.
    //
    // TODO: Take into account the past teammate feedback.
    for f in &fb {
        match students.get_mut(&f.school_username) {
            None => {
                eprintln!("Student feedback includes username {} which isn't represented in the classification.", f.school_username);
                continue;
            }
            Some(ref mut s) => {
                for v in &[f.veto0.as_ref(), f.veto1.as_ref(), f.veto2.as_ref()] {
                    if let Some(vetoed) = v {
                        let v_sanitized = vetoed.to_string().trim().to_ascii_lowercase();
                        if student_classification.get(&v_sanitized).is_none() {
                            eprintln!(
                                "Student {} provided invalid student {} as veto.",
                                s.school_username, v_sanitized
                            );
                        } else {
                            s.vetos.push(v_sanitized);
                        }
                    }
                }
                // only keep the last three
                s.vetos = s
                    .vetos
                    .iter()
                    .unique()
                    .rev()
                    .take(3)
                    .map(|v| v.clone())
                    .collect();

                for w in &[f.want0.as_ref(), f.want1.as_ref(), f.want2.as_ref()] {
                    if let Some(wanted) = w {
                        let w_sanitized = wanted.to_string().trim().to_ascii_lowercase();
                        if student_classification.get(&w_sanitized).is_none() {
                            eprintln!(
                                "Student {} provided invalid student {} as wanted teammate.",
                                s.school_username, w_sanitized
                            );
                        } else {
                            s.wants.push(w_sanitized);
                        }
                    }
                }
                s.wants = s
                    .wants
                    .iter()
                    .unique()
                    .rev()
                    .take(3)
                    .map(|w| w.clone())
                    .collect();
            }
        }
    }

    // move the vetos and wants into the core sets, the add the
    // students attracted to, and repulsed from each student
    for (_, s) in &mut students {
        for vs in &mut s.vetos {
            s.repulsors.insert(vs.clone());
        }
        for ws in &mut s.wants {
            s.attractors.insert(ws.clone());
        }

        for class in student_classification.get(&s.school_username).unwrap() {
            if let Some(att_class) = attractor_class.get(&class.clone()) {
                s.attractors = s.attractors.union(att_class).map(|a| a.clone()).collect();
            } else {
                eprintln!(
                    "Class {} specified for student {}, but not in the relations csv",
                    class, s.school_username
                );
            }
            if let Some(rep_class) = repulsor_class.get(&class.clone()) {
                s.repulsors = s.repulsors.union(rep_class).map(|a| a.clone()).collect();
            } else {
                eprintln!(
                    "Class {} specified for student {}, but not in the relations csv",
                    class, s.school_username
                );
            }
        }
    }

    students
}

type StudentAssignment = Vec<Vec<Username>>;

fn validate_assignment(students: &Students, assignment: &StudentAssignment) -> isize {
    let mut goodness = 0;
    for a in assignment.iter() {
        for s1 in a.iter() {
            for s2 in a.iter() {
                if s1 == s2 {
                    continue;
                }
                let s = students.get(s1).unwrap();
                if s.repulsors.contains(s2) {
                    return -1;
                }
                if s.attractors.contains(s2) {
                    goodness = goodness + 1;
                }
            }
        }
    }

    goodness
}

fn solve(students: &Students) -> Option<StudentAssignment> {
    let mut draft: Vec<Username> = students.iter().map(|(s, _)| s.clone()).collect();
    let mut best = None;
    let mut highest = -1;
    let mut rng = thread_rng();
    let teamsz = 2;

    for _ in 0..(2 as usize).pow(20) {
        let mut assignment = Vec::new();

        draft.shuffle(&mut rng);
        for (n, s) in draft.iter().enumerate() {
            if n % teamsz == 0 {
                assignment.push(Vec::new());
            }
            assignment[n / teamsz].push(s.clone());
        }
        let score = validate_assignment(&students, &assignment);

        if score > highest {
            best = Some(assignment.clone());
            highest = score;
        }
    }

    best
}

fn main() -> anyhow::Result<()> {
    if env::args_os().len() != 4 {
        println!("Usage: {} student_feedback.csv student_classifications.csv classification_relations.csv\nwhere all csv files are tab-delimited and can have arbitrary names.", env::args_os().nth(0).unwrap().to_str().unwrap());
        anyhow::anyhow!("Incorrect number of arguments");
    }
    let (fb, sc, rel) = parse()?;

    let ss = student_matrix(fb, sc, rel);

    let out = solve(&ss);

    if let Some(sol) = out {
        for t in sol.iter() {
            for m in t {
                print!("{},", m);
            }
            print!("\n");
        }
    } else {
        println!("Could not find an assignment that avoids the negative associations");
    }

    Ok(())
}
