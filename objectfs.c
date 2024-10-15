#define FUSE_USE_VERSION 31

#include <errno.h>
#include <fuse.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>

#include <arpa/inet.h>
#include <netdb.h>
#include <netinet/in.h>
#include <openssl/hmac.h>
#include <openssl/sha.h>
#include <sys/socket.h>

#define AWS_CREDS_FILE ".aws/credentials"
#define AWS_CONFIG_FILE ".aws/config"
#define KEEP_FILE ".keep"

#define BUCKET "fuse-tmp" // TODO
#define BUCKET_HOST "fuse-tmp.s3.amazonaws.com" // TODO

#define info(fmt, ...) fprintf(stdout, "INFO: %s:%d: " fmt, __FILE__, __LINE__, ##__VA_ARGS__)
#define error(fmt, ...) fprintf(stderr, "ERROR: %s:%d: " fmt, __FILE__, __LINE__, ##__VA_ARGS__)

#ifdef DEBUG
    #define debug(fmt, ...) fprintf(stderr, "DEBUG: %s:%d: " fmt, __FILE__, __LINE__, ##__VA_ARGS__)
#else
    #define debug(fmt, ...)
#endif

struct awscreds {
    char *key;
    char *secret;
    char *region;
};

/**
 * getawscreds
 * 
 * @param creds: A pointer to the struct of which `key` and `secret` will be allocated to
 * @return: 0 on success, -1 on failure
 * 
 * Note: The caller must free the memory allocated for `key` and `secret`
 */
int getawscreds(struct awscreds *creds) {
    const char *home = getenv("HOME");

    char credspath[1024];
    int res = snprintf(
        credspath,
        sizeof(credspath),
        "%s/%s",
        home,
        AWS_CREDS_FILE
    );
    if(res < 0) {
        error("failed to build aws credentials filepath\n");
        return -1;
    }

    debug("looking for aws credentials file at: %s\n", credspath);

    FILE *credsfile = fopen(credspath, "rb");
    if (credsfile == NULL) {
        error("failed to read aws credentials file\n");
        return -1; 
    }

    fseek(credsfile, 0, SEEK_END);
    long filesize = ftell(credsfile);
    fseek(credsfile, 0, SEEK_SET);

    char *buf = (char *) malloc(filesize + 1);
    if (buf == NULL) {
        error("failed to allocate memory to read aws credentials file\n");
        fclose(credsfile);
        return -1; 
    }

    size_t bytesRead = fread(buf, 1, filesize, credsfile);
    buf[bytesRead] = '\0';
    
    fclose(credsfile);

    char key[256];
    char secret[256];

    char *line = strtok((char *) buf, "\n");
    while(line != NULL) {
        if(strstr(line, "aws_access_key_id") != NULL) {
            sscanf(line, "aws_access_key_id = %s", key);
        } else if(strstr(line, "aws_secret_access_key") != NULL) {
            sscanf(line, "aws_secret_access_key = %s", secret);
        }

        line = strtok(NULL, "\n");
    }

    // TODO check key/secret were found
    creds->key = strdup(key);
    creds->secret = strdup(secret);

    free(buf);

    return 0;
}

/**
 * getawsconfig
 * 
 * @param creds: A pointer to the struct of which `region` will be allocated to
 * @return: 0 on success, -1 on failure
 * 
 * Note: The caller must free the memory allocated for `region`
 */
int getawsconfig(struct awscreds *creds) {
    const char *home = getenv("HOME");

    char configpath[1024];
    int res = snprintf(
        configpath,
        sizeof(configpath),
        "%s/%s",
        home,
        AWS_CONFIG_FILE
    );
    if(res < 0) {
        error("failed to build aws config filepath\n");
        return -1;
    }

    FILE *configfile = fopen(configpath, "rb");
    if (configfile == NULL) {
        error("failed to read aws config file\n");
        return -1; 
    }

    fseek(configfile, 0, SEEK_END);
    long filesize = ftell(configfile);
    fseek(configfile, 0, SEEK_SET);

    char *buf = (char *) malloc(filesize + 1);
    if (buf == NULL) {
        error("failed to allocate memory to read aws config file\n");
        fclose(configfile);
        return -1; 
    }

    size_t bytesRead = fread(buf, 1, filesize, configfile);
    buf[bytesRead] = '\0';
    
    fclose(configfile);

    char region[256];

    char *line = strtok((char *) buf, "\n");
    while(line != NULL) {
        if(strstr(line, "region") != NULL) {
            sscanf(line, "region = %s", region);
        } 

        line = strtok(NULL, "\n");
    }
    
    creds->region = strdup(region);

    free(buf);

    return 0;
}

void tohex(unsigned char *input, size_t len, char *output) {
    for (int i = 0; i < len; i++) {
        sprintf(output + (i * 2), "%02x", input[i]);
    }
    output[len * 2] = '\0';
}

void hmac_sha256(const char *key, const char *data, unsigned char *output) {
    #pragma GCC diagnostic push
    #pragma GCC diagnostic ignored "-Wdeprecated-declarations"

    unsigned int len = 32;
    HMAC_CTX *ctx = HMAC_CTX_new();
    HMAC_Init_ex(ctx, key, strlen(key), EVP_sha256(), NULL);
    HMAC_Update(ctx, (unsigned char*) data, strlen(data));
    HMAC_Final(ctx, output, &len);
    HMAC_CTX_free(ctx);

    #pragma GCC diagnostic pop
}

int awstime(char *timestamp, size_t bufsize) {
    size_t tssize = 20;

    if(bufsize < tssize) {
        error("buffer size too small for timestamp. must be at least: %zu\n", tssize);
        return -1;
    }

    time_t now = time(NULL);
    if(now ==((time_t) -1)) {
        error("failed to get current time\n");
        return -1;
    }

    struct tm *utc = gmtime(&now);
    if(!utc) {
        error("failed to convert time to utc\n");
        return -1;
    }

    if(strftime(timestamp, tssize, "%Y%m%dT%H%M%SZ", utc) == 0) {
        error("failed to format timestamp\n");
        return -1;
    }

    return 0;
}

int awsdate(char *timestamp, size_t bufsize) {
    size_t tssize = 9;

    if(bufsize < tssize) {
        error("buffer size too small for date. must be at least: %zu\n", tssize);
        return -1;
    }

    time_t now = time(NULL);
    if(now ==((time_t) -1)) {
        error("failed to get current time\n");
        return -1;
    }

    struct tm *utc = gmtime(&now);
    if(!utc) {
        error("failed to convert time to utc\n");
        return -1;
    }

    if(strftime(timestamp, tssize, "%Y%m%d", utc) == 0) {
        error("failed to format dat\n");
        return -1;
    }

    return 0;
}

int bucketconnect() {
    int sock;
    if((sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)) < 0) {
        error("failed to create socket\n");
        return -1;
    }

    // get hostname ip
    struct hostent *h;
    if((h = gethostbyname(BUCKET_HOST)) == NULL) {
        error("failed to get ip of hostname\n");
        return -1;
    }
    char *ip = inet_ntoa(*((struct in_addr *) h->h_addr_list[0]));

    // set sin_addr and sin_port
    struct sockaddr_in *remote = (struct sockaddr_in *) malloc(sizeof(struct sockaddr_in *));
    remote->sin_family = AF_INET;
    int res;
    if((res = inet_pton(AF_INET, ip, (void *) (&(remote->sin_addr.s_addr)))) <= 0) {
        error("failed to set sin_addr\n");
        return -1;
    }
    remote->sin_port = htons(80);

    // connect
    if(connect(sock, (struct sockaddr *) remote, sizeof(struct sockaddr)) < 0) {
        error("failed to connect to bucket\n");
        return -1;
    }

    return sock;
}

int getawssignature(char *signature, size_t bufsize, struct awscreds *creds, char *date, char *timestamp) {
    size_t signaturesize = SHA256_DIGEST_LENGTH * 2 + 1;
    if(bufsize < signaturesize) {
        error("buffer size too small for signature. must be at least: %zu\n", signaturesize);
        return -1;
    }

    // create aws canonical request
    char canonical[1024];
    int res = snprintf(
        canonical, 
        sizeof(canonical),
        "GET\n"
        "/\n"
        "encoding-type=url&list-type=2&prefix=\n"
        "host:%s.s3.%s.amazonaws.com\n"
        "x-amz-content-sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n"
        "x-amz-date:%s\n"
        "\n"
        "host;x-amz-content-sha256;x-amz-date\n"
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        BUCKET,
        creds->region,
        timestamp
    );
    if(res < 0) {
        error("failed to build aws canonical request\n");
        return -1;
    }

    debug("aws canonical req:\n%s\n", canonical);

    // hash/hex the canonical req
    unsigned char canonicalhash[SHA256_DIGEST_LENGTH];
    SHA256((unsigned char*) canonical, strlen(canonical), canonicalhash);

    char canonicalhex[SHA256_DIGEST_LENGTH * 2 + 1];
    tohex(canonicalhash, SHA256_DIGEST_LENGTH, canonicalhex);

    // create the string that needs signed
    char tosign[1024];
    snprintf(
        tosign, 
        sizeof(tosign),
        "AWS4-HMAC-SHA256\n"
        "%s\n"
        "%s/%s/s3/aws4_request\n"
        "%s", 
        timestamp, 
        date,
        creds->region,
        canonicalhex
    );

    debug("to sign:\n%s\n", tosign);

    // create the signer
    char kdate[32], kregion[32], kservice[32], signer[32];
    char secret[256];
    res = snprintf(
        secret,
        sizeof(secret),
        "AWS4%s",
        creds->secret
    );
    if(res < 0) {
        error("failed to build secret key\n");
        return -1;
    }

    hmac_sha256(secret, date, (unsigned char*) kdate);
    hmac_sha256(kdate, creds->region, (unsigned char*) kregion);
    hmac_sha256(kregion, "s3", (unsigned char*) kservice);
    hmac_sha256(kservice, "aws4_request", (unsigned char*) signer);

    // hash/hex the string to sing using the signer
    unsigned char signedhash[SHA256_DIGEST_LENGTH];
    hmac_sha256(signer, tosign, signedhash);

    tohex(signedhash, SHA256_DIGEST_LENGTH, signature);

    return 0;
}

static int object_getattr(
    const char *path, 
    struct stat *stbuf, 
    struct fuse_file_info *fi
) {
    debug("`object_getattr` called for: %s\n", path);

    // stbuf->st_uid = getuid();
    // stbuf->st_gid = getgid();
    // stbuf->st_atime = time(NULL);
    // stbuf->st_mtime = time(NULL);
    // if(strcmp(path, "/") == 0 || isdir(path)) {
    //     stbuf->st_mode = __S_IFDIR | 0755;
    //     stbuf->st_nlink = 2;
    // } else if(isfile(path)) {
    //     stbuf->st_mode = __S_IFREG | 0644;
    //     stbuf->st_nlink = 1;
    //     stbuf->st_size = 1024;
    // } else {
    //     return -ENOENT;
    // }

    return 0;
}

static int object_readdir(
    const char *path, 
    void *buf, 
    fuse_fill_dir_t filler, 
    off_t offset, 
    struct fuse_file_info *fi, 
    enum fuse_readdir_flags flags
) {
    debug("`object_readdir` called for: %s\n", path);


    return 0;
}

static int object_read(
    const char *path, 
    char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    debug("`object_read` called for: %s\n", path);

    // memcpy(buf, content + offset, size);

    return 0;
}

static int object_mkdir(
    const char *path, 
    mode_t mode
) {
    debug("`object_mkdir` called for: %s\n", path);

    return 0;
}

static int object_rmdir(const char *path) {
    debug("`object_rmdir` called for: %s\n", path);

    return 0;
}

static int object_mknod(
    const char *path, 
    mode_t mode, 
    dev_t rdev
) {
    debug("`object_mknod` called for: %s\n", path);

    return 0;
}

static int object_unlink(const char *path) {
    debug("`object_unlink` called for: %s\n", path);

    return 0;
}

static int object_write(
    const char *path, 
    const char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    debug("`object_write` called for: %s\n", path);

    return size;
}

static const struct fuse_operations ops = {
    .getattr = object_getattr,
    .readdir = object_readdir,
    .read = object_read,
    .mkdir = object_mkdir,
    .rmdir = object_rmdir,
    .mknod = object_mknod,
    .unlink = object_unlink,
    .write = object_write
};

int main(int argc, char *argv[]) {
    struct awscreds creds;
    if(getawscreds(&creds) < 0) {
        error("failed to get aws creds\n");
        return -EIO;
    }  

    if(getawsconfig(&creds) < 0) {
        error("failed to get aws config\n");
        return -EIO;
    }

    debug("aws info:\nAWS key: %s\nAWS secret: %s\nAWS region: %s\n", creds.key, creds.secret, creds.region);

    int sock = bucketconnect();
    if(sock < 0) {
        error("failed to connect to bucket\n");
        return -EIO;
    }

    // create date/times
    char timestamp[20]; 
    if(awstime(timestamp, sizeof(timestamp)) != 0) {
        error("failed to get aws time\n");
        return -EIO;
    }

    char date[20]; 
    if(awsdate(date, sizeof(date)) != 0) {
        error("failed to get aws date\n");
        return -EIO;
    }

    char signature[SHA256_DIGEST_LENGTH * 2 + 1];
    if(getawssignature(signature, sizeof(signature), &creds, date, timestamp) < 0) {
        error("failed to get aws signature\n");
        return -EIO;
    }

    debug("signature: %s\n", signature);

    // create the http req
    char req[1024];
    snprintf(
        req, 
        sizeof(req),
        "GET /?encoding-type=url&list-type=2&prefix= HTTP/1.1\r\n"
        "Host: %s.s3.%s.amazonaws.com\r\n"
        "x-amz-date: %s\r\n"
        "x-amz-content-sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\r\n"
        "Authorization: AWS4-HMAC-SHA256 Credential=%s/%s/%s/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature=%s\r\n\r\n",
        BUCKET,
        creds.region,
        timestamp,
        creds.key,
        date,
        creds.region,
        signature
    );
    
    debug("http req:\n%s\n", req);

    // send the http req
    int sent = 0;
    while(sent < strlen(req)) {
        int res = send(sock, req + sent, strlen(req) - sent, 0);
        if(res == -1) {
            error("failed to send request\n");
            return -EIO;
        }

        sent += res;
    }

    // receive the http response
    char *response = (char*) malloc(0);
	char BUF[BUFSIZ];
	size_t recived_len = 0;
	while((recived_len = recv(sock, BUF, BUFSIZ-1, 0)) > 0) {
        BUF[recived_len] = '\0';
		response = (char*) realloc(response, strlen(response) + strlen(BUF) + 1);
		sprintf(response, "%s%s", response, BUF);
	}

    fprintf(stdout, "%s\n", response);

    close(sock);
    free(creds.key);
    free(creds.secret);
    free(creds.region);

    return 0;
    // return fuse_main(argc, argv, &ops, NULL);
}
