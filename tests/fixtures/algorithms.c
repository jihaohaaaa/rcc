int gcd(int a, int b) {
    while (b != 0) {
        int t = b;
        b = a % b;
        a = t;
    }
    return a;
}

int is_prime(int n) {
    if (n <= 1) return 0;
    for (int i = 2; i * i <= n; i++) {
        if (n % i == 0) return 0;
    }
    return 1;
}

int count_primes(int limit) {
    int count = 0;
    for (int i = 2; i <= limit; i++) {
        if (is_prime(i)) {
            count++;
        }
    }
    return count;
}

int main() {
    int g = gcd(48, 18); // expected: 6
    int primes = count_primes(30); // 2, 3, 5, 7, 11, 13, 17, 19, 23, 29 => 10 primes

    if (g == 6 && primes == 10) {
        return 0;
    } else {
        return 1;
    }
}
