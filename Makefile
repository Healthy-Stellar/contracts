default: build

all: test

test: build
	cargo test

build:
	stellar contract build
	@ls -l target/wasm32v1-none/release/*.wasm

# Build for WASM target
build-wasm:
	cargo build --release --target wasm32-unknown-unknown --workspace

# Optimize all WASM binaries
optimize: build-wasm
	@echo "Optimizing WASM binaries..."
	@mkdir -p target/wasm32-unknown-unknown/release/optimized
	@for wasm in target/wasm32-unknown-unknown/release/*.wasm; do \
		if [ -f "$$wasm" ] && [[ "$$wasm" != *"_optimized.wasm" ]]; then \
			filename=$$(basename "$$wasm"); \
			echo "Optimizing $$filename..."; \
			if command -v wasm-opt >/dev/null 2>&1; then \
				wasm-opt -O4 -o "target/wasm32-unknown-unknown/release/optimized/$$filename" "$$wasm" || \
				soroban contract optimize --wasm "$$wasm" --wasm-out "target/wasm32-unknown-unknown/release/optimized/$$filename"; \
			else \
				soroban contract optimize --wasm "$$wasm" --wasm-out "target/wasm32-unknown-unknown/release/optimized/$$filename"; \
			fi; \
		fi; \
	done
	@echo "Optimization complete. Optimized binaries in target/wasm32-unknown-unknown/release/optimized/"

# Measure WASM binary sizes
measure-sizes: build-wasm
	@bash scripts/measure-wasm-sizes.sh

# Check binary sizes against limits
check-sizes: optimize
	@echo "Checking binary sizes..."
	@bash scripts/check-wasm-sizes.sh

# Profile WASM binaries (detailed analysis)
profile-wasm: build-wasm
	@echo "Profiling WASM binaries..."
	@bash scripts/profile-wasm.sh

# Strip debug symbols from WASM binaries
strip-wasm: build-wasm
	@echo "Stripping debug symbols..."
	@for wasm in target/wasm32-unknown-unknown/release/*.wasm; do \
		if [ -f "$$wasm" ] && [[ "$$wasm" != *"_stripped.wasm" ]]; then \
			filename=$$(basename "$$wasm"); \
			echo "Stripping $$filename..."; \
			wasm-strip "$$wasm" -o "target/wasm32-unknown-unknown/release/$${filename%.wasm}_stripped.wasm" 2>/dev/null || \
			echo "wasm-strip not available, skipping $$filename"; \
		fi; \
	done

fmt:
	cargo fmt --all

clean:
	cargo clean

.PHONY: default all test build build-wasm optimize measure-sizes check-sizes profile-wasm strip-wasm fmt clean
