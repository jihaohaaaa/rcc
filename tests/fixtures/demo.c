#include <stdio.h>

#include "include/my_header.h"

int add(int a, int b) { return a + b; }

int fib(int n) {
  if (n <= 0)
    return 0;
  if (n == 1)
    return 1;
  return fib(n - 1) + fib(n - 2);
}

int get_distance(Point *p) { return p->x + p->y; }

int sum_array(int *arr, int len) {
  int total = 0;
  for (int i = 0; i < len; i++) {
    total += *(arr + i);
  }
  return total;
}

int main() {
  printf("========================================\n");
  printf("   Welcome to rscc (Rust C Compiler)!   \n");
  printf("========================================\n\n");

  // 1. 递归 Fibonacci
  int f10 = fib(10);
  printf("[1] fib(10) = %d (Expected: 55)\n", f10);

  // 2. 结构体与头文件引用
  Point pt;
  pt.x = 12;
  pt.y = 30;
  int dist = get_distance(&pt);
  printf("[2] Struct distance = %d (Expected: %d)\n", dist, MAGIC_NUMBER);

  // 3. 数组与指针
  int nums[ARRAY_LEN];
  for (int i = 0; i < ARRAY_LEN; i++) {
    nums[i] = (i + 1) * 10;
  }
  int sum = sum_array(nums, ARRAY_LEN);
  printf("[3] Array sum = %d (Expected: 150)\n", sum);

  // 4. 控制流与状态枚举
  Status st = STATUS_OK;
  int check = 0;
  if (f10 == 55 && dist == MAGIC_NUMBER && sum == 150) {
    check = 1;
  }

  if (check) {
    printf("All fixture tests passed!\n");
    return (int)st;
  } else {
    return (int)STATUS_ERR;
  }
}
