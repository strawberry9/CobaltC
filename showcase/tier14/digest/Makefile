# Builds the CobaltC library with cobc, then the C demo against it.
#   make          libdigest.a, digest.h (and libcbrt.a beside them), demo
#   make run      runs the demo
#   make test     the library's own `unittest` blocks under coby, then the demo
#   make clean
# COBC, COBY and CC may be given on the command line (make CC=clang run).
COBC ?= ../../../impl/target/release/cobc
COBY ?= ../../../impl/target/release/coby
CC ?= cc
CFLAGS ?= -O2 -Wall
# The link line for this target, as the first line of digest.h states it.
LIBS = -L. -ldigest -lcbrt -lpthread -ldl -lm

all: demo

# One cobc run writes the library, its header and a copy of the runtime.
libdigest.a: digest.cb
	$(COBC) --lib -o libdigest.a digest.cb

demo: main.c libdigest.a
	$(CC) $(CFLAGS) -o demo main.c $(LIBS)

run: demo
	./demo

test: demo
	$(COBY) --unittest digest.cb
	./demo

clean:
	rm -f demo libdigest.a libcbrt.a digest.h

.PHONY: all run test clean
