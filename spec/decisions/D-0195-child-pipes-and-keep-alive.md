# D-0195 — A child's pipes as values of their own; `HttpClient` keep-alive

Status: ACCEPTED (2026-10-09; the owner: "proceed with 1,2 and 3 using your recommendations" — item 3)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0143, D-0173, D-0181
Affects: `impl/src/procio.rs`, `impl/std/process.cb`, `impl/std/http/client.cb`; `spec/21` (4.47.0); `spec/conformance.md`; the guide; `CHG-0223`

## Problem

Two std gaps found by the round-8 programs:

1. **A child's pipes.** A `Child`'s standard input and output belong to one resource that every operation borrows
   exclusively. So no program can write a child's input from one thread while another thread reads its output (or
   another child's): a streaming pipeline between processes, or feeding a child more input than its pipe buffers
   while reading what it writes, cannot be written. `Command::output_with_input` covers only the case where all
   the input is known first.
2. **HTTP connections.** `HttpClient` opens one connection per request (`Connection: close`). A program making
   many requests to one server pays a TCP (and TLS) handshake each time.

## Decisions

1. **Pipes.**
   - `Child::take_input(&mut ch) : Option<ChildInput>` and `Child::take_output(&mut ch) : Option<ChildOutput>`
     move a pipe out of the child into a resource of its own.
   - `ChildInput::write` writes to it; `ChildOutput::read` and `read_line` read from it.
   - Dropping either end closes it.
   - Each end is independent of the `Child` and of the other end, and outlives the `Child`.
   - Each end names one common intent: one stream of a running process.
   - The OS side keeps each taken pipe under a lock of its own, so threads using different ends never wait for
     each other.
2. **Keep-alive, opt-in.**
   - `HttpClient` gains `export bool keep_alive`, false by default, and keeps at most one idle connection.
   - With `keep_alive` set, a connection is kept when the response was HTTP/1.1 without `Connection: close`, its
     body's end was known without the connection's end, and nothing beyond it was read.
   - The next request to the same origin (scheme, host, port) is made on the kept connection.
   - A kept connection that proves closed before any of the response arrives is replaced by a fresh one, once;
     nothing was received, so the retry repeats nothing observable.
   - Opt-in, because programs written for one connection per request (a server that accepts a fixed number of
     connections, as the guide's example does) would otherwise wait forever.

## Not decided here

A pool of several connections, or one per origin; HTTP/2.
