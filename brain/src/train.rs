use std::fs;

use crate::markov::Markov;
use crate::util::*;

// TODO don't panic here
pub fn train(input: &str, output: &str, depth: usize) {
    let data = {
        timeit!("reading {}", input);
        let size = get_file_size(&input).unwrap();
        eprintln!("size: {} KB", size.comma_separate());
        fs::read_to_string(input).expect("read input")
    };

    // the brain is the output file
    let mut markov = Markov::with_depth(depth, &output);
    {
        timeit!("training");
        eprintln!("training with depth: {}", depth);
        markov.train_text(&data);
    }

    {
        timeit!("writing {}", output);
        let data = bincode::serialize(&markov).unwrap();
        fs::write(output, data).unwrap();
        let size = get_file_size(&output).unwrap();
        eprintln!("size: {} KB", size.comma_separate());
    }
}
