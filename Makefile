PORT?=8081
WASM_NAME?=my_renderer
RUST_TARGET?=wasm32-unknown-unknown
SERVE_DIR?=www
SERVE_WASM_DIR?=${SERVE_DIR}/wasm
CARGO_TOOLCHAIN?=+stable
CARGO_WIN?=--bin windowed_demos --features win
CARGO_WEB?=--lib --target=${RUST_TARGET} --features web
CARGO_TEST?=--features win
WASM_BINDGEN_FLAGS?=--target=web --omit-default-module-path --out-dir ${SERVE_WASM_DIR} --out-name index
CARGO_BUILD_COMMAND?=build
CARGO_TARGET_DIR?=build/web

.PHONY: install
install:
	sudo apt install lld
	rustup toolchain install stable-x86_64-unknown-linux-gnu
	rustup $(CARGO_TOOLCHAIN) target add wasm32-unknown-unknown
	cargo install -f wasm-bindgen-cli
	cargo install wasm-opt --locked

.PHONY: cargo_win_debug
cargo_win_debug:
	CARGO_TARGET_DIR=build/win cargo $(CARGO_TOOLCHAIN) $(CARGO_BUILD_COMMAND) $(CARGO_WIN)

.PHONY: cargo_win
cargo_win:
	CARGO_TARGET_DIR=build/win cargo $(CARGO_TOOLCHAIN) $(CARGO_BUILD_COMMAND) $(CARGO_WIN) --release

.PHONY: cargo_debug
cargo_web_debug:
	CARGO_TARGET_DIR=${CARGO_TARGET_DIR} cargo $(CARGO_TOOLCHAIN) $(CARGO_BUILD_COMMAND) $(CARGO_WEB)

.PHONY: cargo_web
cargo_web:
	CARGO_TARGET_DIR=${CARGO_TARGET_DIR} cargo $(CARGO_TOOLCHAIN) $(CARGO_BUILD_COMMAND) $(CARGO_WEB) --release

.PHONY: test_shaders
test_shaders:
	CARGO_TARGET_DIR=build/win cargo $(CARGO_TOOLCHAIN) test $(CARGO_TEST) --locked --no-fail-fast -j 2 -- $(CARGO_TEST_RUN)

.PHONY: test
test:
# wasm-pack test --node
	CARGO_TARGET_DIR=build/win cargo $(CARGO_TOOLCHAIN) test --lib $(CARGO_TEST) --no-fail-fast -j 2 -- $(CARGO_TEST_RUN)

.PHONY: wasm_debug
wasm_debug: cargo_web_debug
#--keep-debug
	wasm-bindgen $(WASM_BINDGEN_FLAGS) ${CARGO_TARGET_DIR}/${RUST_TARGET}/debug/${WASM_NAME}.wasm

.PHONY: wasm
wasm: cargo_web
	wasm-bindgen $(WASM_BINDGEN_FLAGS) ${CARGO_TARGET_DIR}/${RUST_TARGET}/release/${WASM_NAME}.wasm

.PHONY: wasm_ci
wasm_ci: CARGO_FLAGS += --jobs ${MAX_THREADS}
wasm_ci: wasm

.PHONY: wasm_opt
wasm_opt:
	wasm-opt ${SERVE_WASM_DIR}/index_bg.wasm -O2 --dce --output ${SERVE_WASM_DIR}/index_bg.wasm

.PHONY: pdf_link
pdf_link:
	find ${SERVE_DIR}/assets -type l -iname "*softlink*.pdf" -delete && \
	ln -sfn $(shell cd ${SERVE_DIR}/assets && find . -type f -iname "*eng*.pdf" ! -iname "*softlink*") \
		${SERVE_DIR}/assets/__softlink_cv_eng.pdf && \
	ln -sfn $(shell cd ${SERVE_DIR}/assets && find . -type f -iname "*rus*.pdf" ! -iname "*softlink*") \
		${SERVE_DIR}/assets/__softlink_cv_rus.pdf

.PHONY: codegen
codegen:
	echo "\
	{\n\
   	\"git-commit\": \"$(shell git rev-parse HEAD)\",\n\
   	\"git-commit-date\": \"$(shell git show -s --format=%cD)\",\n\
   	\"debug\": $(if $(filter ${BUILD_TYPE},debug),true,false),\n\
   	\"deploy-date\": \"$(shell LANG=en_us_88591 date +'%a, %d %b %Y %H:%M:%S %z %Z')\"\n\
	}" > ${SERVE_DIR}/build-data.json

.PHONY: codegen_debug
codegen_debug:
	BUILD_TYPE=debug $(MAKE) codegen

.PHONY: build_win
build_win: cargo_win_debug

.PHONY: build_debug
build_debug: wasm_debug codegen_debug

# no `wasm_opt`
.PHONY: build_ci
build_ci: wasm_ci codegen pdf_link

.PHONY: build
build: wasm test_shaders codegen pdf_link wasm_opt

.PHONY: try_build_all
try_build_all: CARGO_BUILD_COMMAND=check
try_build_all: test build_win build_debug

# ===== DEVELOPER DEPLOYMENT
.PHONY: kill_server
kill_server:
	(lsof -t -i :${PORT} -s TCP:LISTEN | xargs kill -9) || true

.PHONY: server_webpack
server_webpack: kill_server
	cd www && \
	export SET NODE_OPTIONS=--openssl-legacy-provider && \
	npx webpack-dev-server --port ${PORT}
#webpack-dev-server --port ${PORT}

.PHONY: server_js
server_js: kill_server
	cd www && node server.js ${PORT} &

.PHONY: server_py
server_py: kill_server
	cd www && python3 -m http.server ${PORT}

.PHONY: app_debug
app_debug: build_debug server_py

.PHONY: app
app: build server_py

.PHONY: app_win
app_win: build_win
	CARGO_TARGET_DIR=build/win RUST_LOG=info cargo $(CARGO_TOOLCHAIN) run $(CARGO_WIN)
