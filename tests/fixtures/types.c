// tests/fixtures/types.c
// Integration tests for C types, sizeof, structs, unions, typedefs, enums, globals.

#include <assert.h>

// Global variables
int g_int = 42;
char g_char = 'Z';
int g_arr[3] = {100, 200, 300};

// Typedef and Enum
typedef unsigned long ulong;
typedef int (*BinaryFunc)(int, int);

enum Direction {
    NORTH = 0,
    EAST = 90,
    SOUTH = 180,
    WEST = 270
};

// Structs
struct Vector2D {
    int x;
    int y;
};

struct Player {
    int id;
    struct Vector2D pos;
    char rank;
};

// Union
union DataHolder {
    int i_val;
    char bytes[4];
};

int add_impl(int a, int b) {
    return a + b;
}

int main() {
    // 1. Sizeof primitive types
    assert(sizeof(char) == 1);
    assert(sizeof(short) == 2);
    assert(sizeof(int) == 4);
    assert(sizeof(long) == 8);
    assert(sizeof(void *) == 8);
    assert(sizeof(int *) == 8);
    assert(sizeof(ulong) == 8);

    int test_arr[10];
    assert(sizeof(test_arr) == 40);
    int grid[3][4];
    assert(sizeof(grid) == 48);

    // 2. Chars & String Literals
    char ch = 'A';
    assert(ch == 65);
    char *str = "Hello";
    assert(str[0] == 'H' && str[4] == 'o' && str[5] == '\0');

    // 3. Globals
    assert(g_int == 42);
    assert(g_char == 'Z');
    assert(g_arr[0] + g_arr[1] + g_arr[2] == 600);

    // 4. Structs & Nested Structs
    struct Player p;
    p.id = 7;
    p.pos.x = 15;
    p.pos.y = 25;
    p.rank = 'S';

    assert(p.id == 7);
    assert(p.pos.x == 15);
    assert(p.pos.y == 25);
    assert(p.rank == 'S');

    struct Player *ptr = &p;
    ptr->pos.x = 50;
    assert(p.pos.x == 50);

    // 5. Enums
    enum Direction d1 = NORTH;
    enum Direction d2 = EAST;
    enum Direction d3 = SOUTH;
    enum Direction d4 = WEST;

    assert(d1 == 0);
    assert(d2 == 90);
    assert(d3 == 180);
    assert(d4 == 270);

    // 6. Function Pointer Typedef
    BinaryFunc op = add_impl;
    assert(op(10, 32) == 42);

    // 7. Union
    union DataHolder holder;
    holder.i_val = 0x01020304;
    // Little endian check
    assert(holder.bytes[0] == 0x04);

    return 0;
}
