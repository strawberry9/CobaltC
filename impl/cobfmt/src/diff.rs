// A unified diff of two texts, by lines (for `--diff`): the longest common
// subsequence, then hunks with three lines of context.

pub fn unified(a: &str, b: &str, name: &str) -> String {
    let x: Vec<&str> = a.lines().collect();
    let y: Vec<&str> = b.lines().collect();
    let (n, m) = (x.len(), y.len());
    // lcs[i][j]: the common length of x[i..] and y[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if x[i] == y[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    // The edit script: ' ' kept, '-' removed, '+' added, with line numbers.
    let mut ops: Vec<(char, usize, usize)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && x[i] == y[j] {
            ops.push((' ', i, j));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            ops.push(('+', i, j));
            j += 1;
        } else {
            ops.push(('-', i, j));
            i += 1;
        }
    }
    let mut out = format!("--- {}\n+++ {} (formatted)\n", name, name);
    let ctx = 3;
    let mut k = 0;
    while k < ops.len() {
        if ops[k].0 == ' ' {
            k += 1;
            continue;
        }
        // A hunk: from `ctx` lines before this change to `ctx` after the last
        // change within reach.
        let start = k.saturating_sub(ctx);
        let mut end = k;
        let mut last_change = k;
        while end < ops.len() {
            if ops[end].0 != ' ' {
                last_change = end;
            } else if end - last_change > 2 * ctx {
                break;
            }
            end += 1;
        }
        let end = (last_change + ctx + 1).min(ops.len());
        let a_start = ops[start].1;
        let b_start = ops[start].2;
        let a_len = ops[start..end].iter().filter(|o| o.0 != '+').count();
        let b_len = ops[start..end].iter().filter(|o| o.0 != '-').count();
        out.push_str(&format!("@@ -{},{} +{},{} @@\n", a_start + 1, a_len, b_start + 1, b_len));
        for o in &ops[start..end] {
            let text = match o.0 {
                '+' => y[o.2],
                _ => x[o.1],
            };
            out.push(o.0);
            out.push_str(text);
            out.push('\n');
        }
        k = end;
    }
    out
}
