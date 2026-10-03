#ifndef _UNISTD_H
#define _UNISTD_H

#include <sys/types.h>
#include <stddef.h>

#define STDIN_FILENO  0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

#define R_OK 4
#define W_OK 2
#define X_OK 1
#define F_OK 0

#define _SC_OPEN_MAX  5
#define _SC_PAGESIZE  29
#define _SC_PAGE_SIZE 29

long sysconf(int name);
char ***_NSGetEnviron(void);
extern char **environ;

ssize_t read(int fd, void *buf, size_t count);
ssize_t write(int fd, const void *buf, size_t count);
ssize_t pread(int fd, void *buf, size_t count, off_t offset);
ssize_t pwrite(int fd, const void *buf, size_t count, off_t offset);
int close(int fd);
off_t lseek(int fd, off_t offset, int whence);
int ftruncate(int fd, off_t length);
int fsync(int fd);
int fdatasync(int fd);
int unlink(const char *pathname);
int rmdir(const char *pathname);
char *getcwd(char *buf, size_t size);
int chdir(const char *path);
int access(const char *pathname, int mode);
int dup(int oldfd);
int dup2(int oldfd, int newfd);
int pipe(int pipefd[2]);
int isatty(int fd);
int usleep(useconds_t usec);
unsigned int sleep(unsigned int seconds);
pid_t fork(void);
int execv(const char *path, char *const argv[]);
int execvp(const char *file, char *const argv[]);
pid_t getpid(void);
pid_t getppid(void);
uid_t getuid(void);
gid_t getgid(void);
int geteuid(void);
int getegid(void);

#endif /* _UNISTD_H */
