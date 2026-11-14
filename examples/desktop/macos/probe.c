// First-party disposable App Sandbox packaging probe, never the user's Desk app.
#include <CoreFoundation/CoreFoundation.h>
#include <Security/SecTask.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <libgen.h>
#include <mach-o/dyld.h>
#include <signal.h>
#include <spawn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>
#define FQE_MAX_INPUT 262144
#define FQE_MAX_OUTPUT (4*1024*1024)
static int transfer(int fd, void *buffer, size_t count, int writing) {
    unsigned char *cursor = buffer;
    while (count) {
        ssize_t n = writing ? write(fd, cursor, count) : read(fd, cursor, count);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) return -1;
        cursor += n; count -= (size_t)n;
    }
    return 0;
}
int main(int argc, char **argv) {
    if (argc != 3 || (strcmp(argv[1], "ffi") && strcmp(argv[1], "ipc"))) return 2;
    alarm(10);
    char executable[4096]; uint32_t capacity = sizeof executable;
    if (_NSGetExecutablePath(executable, &capacity)) return 3;
    char *base = dirname(executable), request_path[8192], native_path[8192];
    snprintf(request_path, sizeof request_path, "%s/../Resources/request.json", base);
    FILE *file = fopen(request_path, "rb"); if (!file) return 4;
    unsigned char *input = malloc(FQE_MAX_INPUT + 1); if (!input) return 5;
    size_t length = fread(input, 1, FQE_MAX_INPUT+1, file); fclose(file);
    if (!length || length > FQE_MAX_INPUT) return 6;
    SecTaskRef task = SecTaskCreateFromSelf(NULL);
    CFTypeRef enabled = task ? SecTaskCopyValueForEntitlement(task, CFSTR("com.apple.security.app-sandbox"), NULL) : NULL;
    int sandbox = enabled && CFGetTypeID(enabled) == CFBooleanGetTypeID() && CFBooleanGetValue(enabled);
    if (enabled) CFRelease(enabled); if (task) CFRelease(task);
    // This public source file is deliberately outside the bundle and has no user-selected grant.
    int denied = open(argv[2], O_RDONLY); int outside_blocked = denied < 0;
    if (denied >= 0) close(denied);
    fprintf(stderr, "sandbox_entitled=%d outside_public_file_blocked=%d\n", sandbox, outside_blocked);
    if (!sandbox || !outside_blocked) return 7;
    if (!strcmp(argv[1], "ffi")) {
        snprintf(native_path, sizeof native_path, "%s/../Frameworks/libfqe_bridge.dylib", base);
        void *library = dlopen(native_path, RTLD_NOW|RTLD_LOCAL); if (!library) return 8;
        uintptr_t (*allocate)(uintptr_t) = dlsym(library, "fqe_buffer_new");
        uintptr_t (*size)(uintptr_t) = dlsym(library, "fqe_buffer_len");
        uint32_t (*release)(uintptr_t) = dlsym(library, "fqe_buffer_free");
        uint64_t (*new_task)(void) = dlsym(library, "fqe_task_new");
        uint32_t (*free_task)(uint64_t) = dlsym(library, "fqe_task_free");
        uintptr_t (*call)(uintptr_t,uintptr_t,uint64_t) = dlsym(library, "fqe_call");
        if (!allocate || !size || !release || !new_task || !free_task || !call) return 9;
        uintptr_t address = allocate(length); uint64_t token = new_task();
        if (!address || !token) return 10;
        memcpy((void *)address, input, length);
        uintptr_t result = call(address,length,token), bytes = size(result);
        if (!result || !bytes || bytes > FQE_MAX_OUTPUT) return 11;
        int status = transfer(STDOUT_FILENO,(void *)result,bytes,1);
        release(result); release(address); free_task(token); dlclose(library); free(input);
        return status ? 12 : 0;
    }
    snprintf(native_path, sizeof native_path, "%s/fqe-desktop-peer", base);
    int in[2],out[2]; if (pipe(in) || pipe(out)) return 13;
    posix_spawn_file_actions_t actions; posix_spawn_file_actions_init(&actions);
    posix_spawn_file_actions_adddup2(&actions,in[0],STDIN_FILENO);
    posix_spawn_file_actions_adddup2(&actions,out[1],STDOUT_FILENO);
    for (int i=0;i<2;i++) { posix_spawn_file_actions_addclose(&actions,in[i]);posix_spawn_file_actions_addclose(&actions,out[i]); }
    char *args[]={native_path,NULL},*env[]={NULL}; pid_t child;
    int error=posix_spawn(&child,native_path,&actions,NULL,args,env);
    posix_spawn_file_actions_destroy(&actions); close(in[0]); close(out[1]);
    if (error) {close(in[1]);close(out[0]);return 14;}
    unsigned char header[]={length>>24,length>>16,length>>8,length};
    int status=transfer(in[1],header,4,1) || transfer(in[1],input,length,1);
    free(input);
    if (!status) status=transfer(out[0],header,4,0);
    uint32_t result_length=((uint32_t)header[0]<<24)|((uint32_t)header[1]<<16)|((uint32_t)header[2]<<8)|header[3];
    unsigned char *result=NULL;
    if (!status && result_length && result_length<=FQE_MAX_OUTPUT) {
        result=malloc(result_length);
        status=!result || transfer(out[0],result,result_length,0) || transfer(STDOUT_FILENO,result,result_length,1);
    } else status=1;
    free(result);close(in[1]);close(out[0]);
    if (status) kill(child,SIGKILL);
    int child_status; waitpid(child,&child_status,0);
    return status || !WIFEXITED(child_status) || WEXITSTATUS(child_status) ? 15 : 0;
}
