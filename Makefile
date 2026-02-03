.PHONY: clean

OS := $(shell uname)

all: compiler

compiler: 
	cargo build -r -j8

clean:
	rm -r build/

install: compiler
	@if [ $(OS) == Linux ]; then \
		cp target/release/slimec /usr/local/bin/; \
		chmod 755 /usr/local/bin/slimec; \
	else \
		echo "make install only supports linux"; \
	fi