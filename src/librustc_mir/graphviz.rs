// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use dot;
use rustc::mir::repr::*;
use rustc::middle::ty;
use std::fmt::Debug;
use std::io::{self, Write};
use syntax::ast::NodeId;

/// Write a graphviz DOT graph of a list of MIRs.
pub fn write_mir_graphviz<'a, 't, W, I>(tcx: &ty::TyCtxt<'t>, iter: I, w: &mut W) -> io::Result<()>
where W: Write, I: Iterator<Item=(&'a NodeId, &'a Mir<'a>)> {
    for (&nodeid, mir) in iter {
        try!(writeln!(w, "digraph Mir_{} {{", nodeid));

        // Global graph properties
        try!(writeln!(w, r#"    graph [fontname="monospace"];"#));
        try!(writeln!(w, r#"    node [fontname="monospace"];"#));
        try!(writeln!(w, r#"    edge [fontname="monospace"];"#));

        // Graph label
        try!(write_graph_label(tcx, nodeid, mir, w));

        // Nodes
        for block in mir.all_basic_blocks() {
            try!(write_node(block, mir, w));
        }

        // Edges
        for source in mir.all_basic_blocks() {
            try!(write_edges(source, mir, w));
        }
        try!(writeln!(w, "}}"))
    }
    Ok(())
}

/// Write a graphviz HTML-styled label for the given basic block, with
/// all necessary escaping already performed. (This is suitable for
/// emitting directly, as is done in this module, or for use with the
/// LabelText::HtmlStr from libgraphviz.)
///
/// `init` and `fini` are callbacks for emitting additional rows of
/// data (using HTML enclosed with `<tr>` in the emitted text).
pub fn write_node_label<W: Write, INIT, FINI>(block: BasicBlock,
                                              mir: &Mir,
                                              w: &mut W,
                                              num_cols: u32,
                                              init: INIT,
                                              fini: FINI) -> io::Result<()>
    where INIT: Fn(&mut W) -> io::Result<()>,
          FINI: Fn(&mut W) -> io::Result<()>
{
    let data = mir.basic_block_data(block);

    try!(write!(w, r#"<table border="0" cellborder="1" cellspacing="0">"#));

    // Basic block number at the top.
    try!(write!(w, r#"<tr><td {attrs} colspan="{colspan}">{blk}</td></tr>"#,
                attrs=r#"bgcolor="gray" align="center""#,
                colspan=num_cols,
                blk=block.index()));

    try!(init(w));

    // List of statements in the middle.
    if !data.statements.is_empty() {
        try!(write!(w, r#"<tr><td align="left" balign="left">"#));
        for statement in &data.statements {
            try!(write!(w, "{}<br/>", escape(statement)));
        }
        try!(write!(w, "</td></tr>"));
    }

    // Terminator head at the bottom, not including the list of successor blocks. Those will be
    // displayed as labels on the edges between blocks.
    let mut terminator_head = String::new();
    data.terminator().fmt_head(&mut terminator_head).unwrap();
    try!(write!(w, r#"<tr><td align="left">{}</td></tr>"#, dot::escape_html(&terminator_head)));

    try!(fini(w));

    // Close the table
    writeln!(w, "</table>")
}

/// Write a graphviz DOT node for the given basic block.
fn write_node<W: Write>(block: BasicBlock, mir: &Mir, w: &mut W) -> io::Result<()> {
    // Start a new node with the label to follow, in one of DOT's pseudo-HTML tables.
    try!(write!(w, r#"    {} [shape="none", label=<"#, node(block)));
    try!(write_node_label(block, mir, w, 1, |_| Ok(()), |_| Ok(())));
    // Close the node label and the node itself.
    writeln!(w, ">];")
}

/// Write graphviz DOT edges with labels between the given basic block and all of its successors.
fn write_edges<W: Write>(source: BasicBlock, mir: &Mir, w: &mut W) -> io::Result<()> {
    let terminator = &mir.basic_block_data(source).terminator();
    let labels = terminator.fmt_successor_labels();

    for (&target, label) in terminator.successors().iter().zip(labels) {
        try!(writeln!(w, r#"    {} -> {} [label="{}"];"#, node(source), node(target), label));
    }

    Ok(())
}

/// Write the graphviz DOT label for the overall graph. This is essentially a block of text that
/// will appear below the graph, showing the type of the `fn` this MIR represents and the types of
/// all the variables and temporaries.
fn write_graph_label<W: Write>(tcx: &ty::TyCtxt, nid: NodeId, mir: &Mir, w: &mut W)
-> io::Result<()> {
    try!(write!(w, "    label=<fn {}(", dot::escape_html(&tcx.map.path_to_string(nid))));

    // fn argument types.
    for (i, arg) in mir.arg_decls.iter().enumerate() {
        if i > 0 {
            try!(write!(w, ", "));
        }
        try!(write!(w, "{:?}: {}", Lvalue::Arg(i as u32), escape(&arg.ty)));
    }

    try!(write!(w, ") -&gt; "));

    // fn return type.
    match mir.return_ty {
        ty::FnOutput::FnConverging(ty) => try!(write!(w, "{}", escape(ty))),
        ty::FnOutput::FnDiverging => try!(write!(w, "!")),
    }

    try!(write!(w, r#"<br align="left"/>"#));

    // User variable types (including the user's name in a comment).
    for (i, var) in mir.var_decls.iter().enumerate() {
        try!(write!(w, "let "));
        if var.mutability == Mutability::Mut {
            try!(write!(w, "mut "));
        }
        try!(write!(w, r#"{:?}: {}; // {}<br align="left"/>"#,
                    Lvalue::Var(i as u32), escape(&var.ty), var.name));
    }

    // Compiler-introduced temporary types.
    for (i, temp) in mir.temp_decls.iter().enumerate() {
        try!(write!(w, r#"let mut {:?}: {};<br align="left"/>"#,
                    Lvalue::Temp(i as u32), escape(&temp.ty)));
    }

    writeln!(w, ">;")
}

fn node(block: BasicBlock) -> String {
    format!("bb{}", block.index())
}

fn escape<T: Debug>(t: &T) -> String {
    dot::escape_html(&format!("{:?}", t))
}
