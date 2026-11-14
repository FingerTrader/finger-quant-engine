// First-party transport-failure fixture; never bundled with the product.
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
int main(int argc,char **argv) {
    (void)argc;
    const char *mode=strrchr(argv[0],'/'); mode=mode?mode+1:argv[0];
    unsigned char header[4]; if (fread(header,1,4,stdin)!=4) return 2;
    unsigned length=((unsigned)header[0]<<24)|((unsigned)header[1]<<16)|((unsigned)header[2]<<8)|header[3];
    if (length>262144) return 3;
    while(length--) {if (fgetc(stdin)==EOF) return 4;}
    if (!strcmp(mode,"oversized")) {unsigned char bytes[]={255,255,255,255};fwrite(bytes,1,4,stdout);fflush(stdout);}
    else if (!strcmp(mode,"stderr")) {for(unsigned i=0;i<70000;i++) fputc('x',stderr);fflush(stderr);}
    else if (!strcmp(mode,"partial")) {unsigned char bytes[]={0,0,0,100,'{'};fwrite(bytes,1,5,stdout);fflush(stdout);return 0;}
    else if (!strcmp(mode,"timeout")) {sleep(10);}
    else {
        const char *body=!strcmp(mode,"wrongid")?"{\"format\":\"fqe.desktop-response\",\"schema_version\":1,\"id\":\"wrong\",\"ok\":true,\"body\":{}}":!strcmp(mode,"json")?"invalid-json":"{\"format\":\"fqe.desktop-response\",\"schema_version\":1,\"id\":\"dart.1\",\"ok\":true,\"body\":{}}";
        size_t size=strlen(body);unsigned char bytes[]={size>>24,size>>16,size>>8,size};
        int count=!strcmp(mode,"duplicate")?2:1;
        for(int i=0;i<count;i++){fwrite(bytes,1,4,stdout);fwrite(body,1,size,stdout);fflush(stdout);}
    }
    sleep(10);return 0;
}
