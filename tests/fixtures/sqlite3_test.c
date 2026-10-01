// tests/fixtures/sqlite3_test.c
// Integration test: Compile and run SQLite 3 engine via rscc.

#define NDEBUG 1
#define SQLITE_THREADSAFE 0
#define SQLITE_OMIT_LOAD_EXTENSION 1
#define SQLITE_DISABLE_LFS 1
#define SQLITE_OMIT_DEPRECATED 1


#include "vendor/sqlite/sqlite3.c"

static int callback_count = 0;
static int exec_callback(void *data, int argc, char **argv, char **azColName) {
    callback_count++;
    printf("[SQLite Test] Row: id=%s, name=%s, score=%s\n", argv[0], argv[1], argv[2]);
    return 0;
}

int main() {
    printf("[SQLite Test] SQLite Version: %s\n", sqlite3_libversion());

    sqlite3 *db = NULL;
    int rc = sqlite3_open(":memory:", &db);
    assert(rc == SQLITE_OK);
    assert(db != NULL);

    // 1. Create Table
    char *err_msg = NULL;
    const char *sql_create = "CREATE TABLE students ("
                             "id INTEGER PRIMARY KEY, "
                             "name TEXT NOT NULL, "
                             "score REAL, "
                             "active INTEGER);";
    rc = sqlite3_exec(db, sql_create, NULL, NULL, &err_msg);
    if (rc != SQLITE_OK) {
        fprintf(stderr, "SQL error: %s\n", err_msg);
        sqlite3_free(err_msg);
        assert(0);
    }
    printf("[SQLite Test] 1. Table created successfully.\n");

    // 2. Insert records using sqlite3_exec
    const char *sql_insert = "INSERT INTO students VALUES (1, 'Alice', 98.5, 1);"
                             "INSERT INTO students VALUES (2, 'Bob', 85.0, 1);"
                             "INSERT INTO students VALUES (3, 'Charlie', 72.5, 0);";
    rc = sqlite3_exec(db, sql_insert, NULL, NULL, &err_msg);
    assert(rc == SQLITE_OK);
    printf("[SQLite Test] 2. Multiple rows inserted.\n");

    // 3. Insert using Prepared Statements & Parameter Binding
    sqlite3_stmt *stmt = NULL;
    const char *sql_bind = "INSERT INTO students (id, name, score, active) VALUES (?, ?, ?, ?);";
    rc = sqlite3_prepare_v2(db, sql_bind, -1, &stmt, NULL);
    assert(rc == SQLITE_OK);

    sqlite3_bind_int(stmt, 1, 4);
    sqlite3_bind_text(stmt, 2, "Diana", -1, SQLITE_STATIC);
    sqlite3_bind_double(stmt, 3, 91.0);
    sqlite3_bind_int(stmt, 4, 1);

    rc = sqlite3_step(stmt);
    assert(rc == SQLITE_DONE);
    sqlite3_finalize(stmt);
    printf("[SQLite Test] 3. Prepared statement inserted Diana.\n");

    // 4. Query using sqlite3_exec callback
    callback_count = 0;
    const char *sql_select = "SELECT id, name, score FROM students ORDER BY id;";
    rc = sqlite3_exec(db, sql_select, exec_callback, NULL, &err_msg);
    assert(rc == SQLITE_OK);
    assert(callback_count == 4);
    printf("[SQLite Test] 4. Queried %d rows via callback.\n", callback_count);

    // 5. Query with Prepared Statement & Column extraction
    const char *sql_query = "SELECT id, name, score, active FROM students WHERE score >= ? ORDER BY score DESC;";
    rc = sqlite3_prepare_v2(db, sql_query, -1, &stmt, NULL);
    assert(rc == SQLITE_OK);
    sqlite3_bind_double(stmt, 1, 85.0);

    int rows_found = 0;
    while ((rc = sqlite3_step(stmt)) == SQLITE_ROW) {
        int id = sqlite3_column_int(stmt, 0);
        const unsigned char *name = sqlite3_column_text(stmt, 1);
        double score = sqlite3_column_double(stmt, 2);
        int active = sqlite3_column_int(stmt, 3);
        printf("[SQLite Test] Found honor student: id=%d, name=%s, score=%.1f, active=%d\n", id, name, score, active);
        rows_found++;
    }
    assert(rc == SQLITE_DONE);
    assert(rows_found == 3); // Alice (98.5), Diana (91.0), Bob (85.0)
    sqlite3_finalize(stmt);
    printf("[SQLite Test] 5. Prepared statement query returned %d rows.\n", rows_found);

    // 6. Aggregate calculation (COUNT, AVG, MAX, MIN)
    const char *sql_agg = "SELECT COUNT(*), AVG(score), MAX(score), MIN(score) FROM students;";
    rc = sqlite3_prepare_v2(db, sql_agg, -1, &stmt, NULL);
    assert(rc == SQLITE_OK);
    rc = sqlite3_step(stmt);
    assert(rc == SQLITE_ROW);
    int total_students = sqlite3_column_int(stmt, 0);
    double avg_score = sqlite3_column_double(stmt, 1);
    double max_score = sqlite3_column_double(stmt, 2);
    double min_score = sqlite3_column_double(stmt, 3);
    printf("[SQLite Test] 6. Aggregates: Total=%d, Avg=%.2f, Max=%.1f, Min=%.1f\n", total_students, avg_score, max_score, min_score);
    assert(total_students == 4);
    assert((int)avg_score == 86); // (98.5 + 85 + 72.5 + 91) / 4 = 86.75
    assert((int)max_score == 98);
    assert((int)min_score == 72);
    sqlite3_finalize(stmt);

    // 7. Transactions: Rollback and Commit
    rc = sqlite3_exec(db, "BEGIN TRANSACTION;", NULL, NULL, NULL);
    assert(rc == SQLITE_OK);
    rc = sqlite3_exec(db, "INSERT INTO students VALUES (99, 'Temp', 0.0, 0);", NULL, NULL, NULL);
    assert(rc == SQLITE_OK);
    rc = sqlite3_exec(db, "ROLLBACK;", NULL, NULL, NULL);
    assert(rc == SQLITE_OK);

    // Verify Temp was not saved
    rc = sqlite3_prepare_v2(db, "SELECT COUNT(*) FROM students WHERE id = 99;", -1, &stmt, NULL);
    assert(rc == SQLITE_OK);
    rc = sqlite3_step(stmt);
    assert(rc == SQLITE_ROW);
    assert(sqlite3_column_int(stmt, 0) == 0);
    sqlite3_finalize(stmt);
    printf("[SQLite Test] 7. Transaction rollback verified.\n");

    // 8. Close database
    rc = sqlite3_close(db);
    assert(rc == SQLITE_OK);

    printf("[SQLite Test] ALL SQLITE TESTS PASSED!\n");
    return 0;
}
