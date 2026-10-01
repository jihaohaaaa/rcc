#ifndef MY_HEADER_H
#define MY_HEADER_H

#define MAGIC_NUMBER 42
#define ARRAY_LEN 5

typedef struct Point {
    int x;
    int y;
} Point;

typedef enum Status {
    STATUS_OK = 0,
    STATUS_ERR = 1,
} Status;

int add(int a, int b);
int fib(int n);
int get_distance(Point *p);

#endif // MY_HEADER_H
