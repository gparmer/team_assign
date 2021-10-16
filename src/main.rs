use anyhow;
use csv::Reader;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::Read;

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

fn parse<R: Read>(
    mut fb_rdr: Reader<R>,
    mut sc_rdr: Reader<R>,
    mut rel_rdr: Reader<R>,
) -> anyhow::Result<(
    Vec<StudentFeedback>,
    Vec<StudentClassification>,
    Vec<ClassificationRelations>,
)> {
    let mut fb = Vec::new();
    let mut sc = Vec::new();
    let mut rel = Vec::new();

    for result in fb_rdr.deserialize() {
        let mut fb_record: StudentFeedback = result?;

        fb_record.school_username = fb_record.school_username.trim().to_ascii_lowercase();
        fb_record.github_username = fb_record.github_username.trim().to_ascii_lowercase();
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
    github_username: GhUsername,
    attractors: HashSet<Username>,
    classification_attractors: HashSet<Username>,
    repulsors: HashSet<Username>,
    wants: Vec<Username>,
    vetos: Vec<Username>,
    ok_solo: bool,
}

impl Student {
    fn new(email: String, school_username: Username, github_username: GhUsername) -> Self {
        Student {
            _email: email,
            school_username,
            github_username: github_username,
            attractors: HashSet::new(),
            classification_attractors: HashSet::new(),
            repulsors: HashSet::new(),
            vetos: Vec::new(),
            wants: Vec::new(),
	    ok_solo: false,
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
    let mut gh2school: HashMap<GhUsername, Username> = HashMap::new();

    fn insert_classification(
        s: &Username,
        c: &String,
        classification_students: &mut HashMap<String, HashSet<Username>>,
        student_classification: &mut HashMap<Username, HashSet<String>>,
    ) {
        if c == "" {
            return;
        }

        if !classification_students.contains_key(&c.to_string()) {
            classification_students.insert(c.to_string(), HashSet::new());
        }
        // unwrap valid given the insert above
        classification_students
            .get_mut(&c.clone())
            .unwrap()
            .insert(s.to_string());

	if !student_classification.contains_key(&s.clone()) {
            student_classification.insert(s.clone(), HashSet::new());
	}
	// unwrap valid due to insertion above
        student_classification.get_mut(s).unwrap().insert(c.clone());
    }

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
            insert_classification(
                &s.school_username.clone(),
                &c,
                &mut classification_students,
                &mut student_classification,
            );
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

    // Populate the map from github username to school username
    fb.iter().for_each(|f| {
        gh2school.insert(f.github_username.clone(), f.school_username.clone());
    });

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
		// Do we have someone OK working solo?
		if f.solo == "Yes" {
		    s.ok_solo = true;
		} else if f.solo == "No" {
		    s.ok_solo = false;
		}

		let mut fb_vetos = Vec::new();
                let mut fb_wants = Vec::new();

                if let Some(ref last_gh_username) = f.last_teammate_github_username {
                    let schoolname_valid = student_classification.get(last_gh_username).is_some();
                    let last_teammate = gh2school.get(last_gh_username).or_else(|| {
                        if schoolname_valid {
                            // did they use school username instead of github?
                            Some(&last_gh_username)
                        } else {
                            None
                        }
                    });

                    if let Some(last_teammate) = last_teammate {
                        // TODO move this to a csv as well
                        let comment_mapping = [
                            ("Fantastic teammate.", (Some("strong"), false, false)),
                            ("Would love to work with them again.", (None, true, false)),
                            ("Both of us working together were greater than the sum of the individuals.", (Some("helpful"), false, false)),
                            ("Was not technically capable of pulling their weight.", (None, false, true)),
                            ("Did not have a great experience with them.", (Some("watch"), false, true)),
                            ("Did not put in sufficent effort.", (Some("lazy"), false, true)),
                            ("Was not sufficiently responsive.", (Some("lazy"), false, true)),
                            ("Procrastinated.", (Some("lazy"), false, true)),
                            ("Disrespectful.", (Some("problem"), false, true)),
                            ("Abusive.", (Some("problem"), false, true)),
			];
                        let mut comment_map = HashMap::new();
                        for (c, m) in &comment_mapping {
                            comment_map.insert(c, m.clone());
                        }

                        for last_fb in f
                            .last_teammate_feedback
                            .as_ref()
                            .unwrap_or(&String::from(""))
                            .split(",")
                            .map(|s| String::from(s))
                        {
                            let c = comment_map.get(&last_fb.trim());
                            assert_eq!(c.is_some(), true);
                            let (c, want, veto) = c.unwrap();
                            if *veto {
                                fb_vetos.push(last_teammate.to_string());
                            }
                            if *want {
                                fb_wants.push(last_teammate.to_string());
                            }
                            if let Some(c) = c {
                                insert_classification(
                                    &last_teammate,
                                    &c.to_string(),
                                    &mut classification_students,
                                    &mut student_classification,
                                );
                            }
                        }
                    }
                }

                // Process vetos
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
                s.vetos.append(&mut fb_vetos);

                // process wanted partners
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
                s.wants.append(&mut fb_wants);
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
                s.classification_attractors = s
                    .classification_attractors
                    .union(att_class)
                    .map(|a| a.clone())
                    .collect();
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

#[derive(Clone)]
struct Team {
    members: Vec<Username>,
    score: usize,
}

impl Team {
    fn new() -> Self {
        Team {
            members: Vec::new(),
	    score: 0
        }
    }
}

type StudentAssignment = Vec<Team>;

fn validate_assignment(students: &Students, assignment: &mut StudentAssignment) -> Option<usize> {
    let mut scores: Vec<usize> = Vec::new();
    let mut goodness: usize = 0;

    for a in assignment.iter() {
        let mut team_goodness = 0;

        for s1 in a.members.iter() {
            for s2 in a.members.iter() {
                if s1 == s2 {
                    continue;
                }
                let s = students.get(s1).unwrap();
                if s.repulsors.contains(s2) {
                    return None;
                }
                if s.attractors.contains(s2) {
                    team_goodness += 3; // heavily prefer the explicit attractors
                } else if s.classification_attractors.contains(s2) {
                    team_goodness += 1;
                }
            }
        }
        scores.push(team_goodness);
        goodness += team_goodness;
    }

    let mut cnt = 0;
    for a in assignment.iter_mut() {
	a.score = scores[cnt];
	cnt += 1;
    }

    Some(goodness)
}

fn solve(students: &Students) -> Option<StudentAssignment> {
    let mut draft: Vec<Username> = students.iter().map(|(s, _)| s.clone()).collect();
    let mut best = None;
    let mut highest = 0;
    let mut rng = thread_rng();
    let teamsz = 2;

    // If the # of students doesn't match up with class size / teams
    // size, identify who is fine with doing it solo, or in smaller
    // teams?
    let mut nstragglers = draft.len() % teamsz;
    let mut solos = Vec::new();

    if nstragglers != 0 {
	let mut draft_pruned: Vec<Username> = Vec::new();

	draft.shuffle(&mut rng);
	for n in &draft {
	    let s = students.get(n).unwrap();

	    if s.ok_solo && nstragglers > 0 {
		solos.push(n.clone());
		nstragglers -= 1;
	    } else {
		draft_pruned.push(n.clone());
	    }
	}

	draft = draft_pruned;
    }

    // For the rest of the students, generate random assignments, and
    // find out which has the highest "score".
    for _ in 0..(2 as usize).pow(20) {
        let mut assignment = Vec::new();

        draft.shuffle(&mut rng);
        for (n, s) in draft.iter().enumerate() {
            if n % teamsz == 0 {
                assignment.push(Team::new());
            }
            assignment[n / teamsz].members.push(s.clone());
        }
        if let Some(score) = validate_assignment(&students, &mut assignment) {
            if score > highest {
		best = Some(assignment.clone());
		highest = score;
            }
	}
    }

    if let Some(ref mut b) = best {
	for s in &solos {
	    let mut t = Team::new();

	    t.members.push(s.clone());
	    b.push(t);
	}
    }

    best
}

fn file_arg_to_reader(argn: usize) -> anyhow::Result<csv::Reader<File>> {
    let file_path = env::args_os()
        .nth(argn)
        .ok_or(anyhow::anyhow!("Argument {} not provided.", argn))?;
    let file = File::open(file_path)?;

    Ok(csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .comment(Some(b'#'))
        .flexible(true)
        .from_reader(file))
}

fn main() -> anyhow::Result<()> {
    if env::args_os().len() != 4 {
        println!("Usage: {} student_feedback.csv student_classifications.csv classification_relations.csv\nwhere all csv files are tab-delimited and can have arbitrary names.", env::args_os().nth(0).unwrap().to_str().unwrap());
        anyhow::anyhow!("Incorrect number of arguments");
    }
    let (fb, sc, rel) = parse(
        file_arg_to_reader(1)?,
        file_arg_to_reader(2)?,
        file_arg_to_reader(3)?,
    )?;

    let ss = student_matrix(fb, sc, rel);

    let out = solve(&ss);

    if let Some(sol) = out {
        for t in sol.iter() {
	    let mut team_name = String::from("team");
            for m in &t.members {
                print!("{},", m);
		team_name = format!("{}_{}", team_name, m);
            }
	    print!("{},", team_name);
            print!("{}\n", t.score);
        }
    } else {
        println!("Could not find an assignment that avoids the negative associations");
    }

    Ok(())
}
