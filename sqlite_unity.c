// sqlite_unity.c: Standalone SQLite3 CLI interpreter & database engine compiled with rscc
#define NDEBUG 1
#define SQLITE_THREADSAFE 0
#define SQLITE_OMIT_LOAD_EXTENSION 1
#define SQLITE_DISABLE_LFS 1
#define SQLITE_OMIT_DEPRECATED 1

#include "vendor/sqlite/sqlite3.c"
#include "vendor/sqlite/shell.c"
