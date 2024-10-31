#define FUSE_USE_VERSION 35

#include <iostream>
#include <string.h>

#include <aws/core/auth/AWSCredentialsProviderChain.h>
#include <aws/core/Aws.h>
#include <aws/s3/model/DeleteObjectRequest.h>
#include <aws/s3/model/GetObjectRequest.h>
#include <aws/s3/model/HeadObjectRequest.h>
#include <aws/s3/model/ListObjectsV2Request.h>
#include <aws/s3/model/PutObjectRequest.h>
#include <aws/s3/S3Client.h>
#include <spdlog/spdlog.h>

extern "C" {
    #include <fuse3/fuse.h>
}

#define BUCKET "fuse-tmp"
#define KEEP_FILE ".keep"

Aws::S3::S3Client* s3Client = nullptr;

int object_getattr(
    const char *path, 
    struct stat *stbuf, 
    struct fuse_file_info *fi
) {
    spdlog::debug("`_getattr` called for: {}", path);
    if(!s3Client) {
        spdlog::error("client uninitialized");
        return -EIO;
    }

	memset(stbuf, 0, sizeof(struct stat));

	if(strcmp(path, "/") == 0) {
		stbuf->st_mode = S_IFDIR | 0755;
		stbuf->st_nlink = 2;
	} else {
        path++;

        Aws::S3::Model::HeadObjectRequest req;
        req.WithBucket(BUCKET).WithKey(path);

        auto res = s3Client->HeadObject(req);
        if(!res.IsSuccess()) {
            if(res.GetError().GetErrorType() == Aws::S3::S3Errors::NO_SUCH_KEY) {
                spdlog::warn("not found: {}", path);
                return -ENOENT;
            }

            spdlog::error("failed to head object: {}: {}", path, res.GetError().GetMessage());
            return -EIO;
        }

		stbuf->st_mode = S_IFREG | 0644;
		stbuf->st_nlink = 1;
		stbuf->st_size = res.GetResult().GetContentLength();
        stbuf->st_mtime = res.GetResult().GetLastModified().SecondsWithMSPrecision();
        stbuf->st_uid = 0; 
        stbuf->st_gid = 0; 
	} 

    return 0;
}

int object_mknod(
    const char *path, 
    mode_t mode, 
    dev_t rdev
) {
    spdlog::debug("`_mknod` called for: {}", path);
    if(!s3Client) {
        spdlog::error("client uninitialized");
        return -EIO;
    }

    path++;

    Aws::S3::Model::PutObjectRequest req;
    req.WithBucket(BUCKET).WithKey(path);

    auto stream = Aws::MakeShared<Aws::StringStream>("putobjinstream");
    *stream << "";
    req.SetBody(stream);

    auto res = s3Client->PutObject(req);
    if(!res.IsSuccess()) {
        spdlog::error("failed to put object at: {}: {}", path, res.GetError().GetMessage());
        return -EIO;
    }

    return 0;
}

int object_unlink(const char *path) {
    spdlog::debug("`_unlink` called for: {}", path);
    if(!s3Client) {
        spdlog::error("client uninitialized");
        return -EIO;
    }

    path++;

    Aws::S3::Model::DeleteObjectRequest req;
    req.WithBucket(BUCKET).WithKey(path);

    auto res = s3Client->DeleteObject(req);
    if(!res.IsSuccess()) {
        if(res.GetError().GetErrorType() == Aws::S3::S3Errors::NO_SUCH_KEY) {
            spdlog::warn("not found: {}", path);
            return -ENOENT;
        }

        return -EIO;
    }

    return 0;
}

int object_write(
    const char *path, 
    const char *buf, 
    size_t size,
	off_t offset, 
    struct fuse_file_info *fi
) {
    spdlog::debug("`_write` called for: {}, size: {}, offset: {}", path, size, offset);
    if(!s3Client) {
        spdlog::error("client uninitialized");
        return -EIO;
    }

    path++;

    Aws::S3::Model::HeadObjectRequest headreq;
    headreq.WithBucket(BUCKET).WithKey(path);

    auto headers = s3Client->HeadObject(headreq);
    if(!headers.IsSuccess()) {
        if(headers.GetError().GetErrorType() == Aws::S3::S3Errors::NO_SUCH_KEY) {
            spdlog::warn("not found: {}", path);
            return -EINVAL;
        }

        spdlog::error("failed to head object: {}: {}", path, headers.GetError().GetMessage());
        return -EIO;
    }

    Aws::S3::Model::PutObjectRequest putreq;
    putreq.WithBucket(BUCKET).WithKey(path);

    auto stream = Aws::MakeShared<Aws::StringStream>("putobjinstream");
    *stream << buf;
    putreq.SetBody(stream);

    auto putres = s3Client->PutObject(putreq);
    if(!putres.IsSuccess()) {
        spdlog::error("failed to put object at: {}: {}", path, putres.GetError().GetMessage());
        return -EIO;
    }

    return 0;
}

int object_readdir(
    const char *path, 
    void *buf, 
    fuse_fill_dir_t filler, 
    off_t offset, 
    struct fuse_file_info *fi, 
    enum fuse_readdir_flags flags
) {
    spdlog::debug("`_readdir` called for: {}", path);
    if(!s3Client) {
        spdlog::error("client uninitialized");
        return -EIO;
    }

    filler(buf, ".", nullptr, 0, FUSE_FILL_DIR_PLUS);
    filler(buf, "..", nullptr, 0, FUSE_FILL_DIR_PLUS);

    Aws::S3::Model::ListObjectsV2Request req;
    req.SetBucket(BUCKET);
    
    Aws::String contok;
    do {
        if(!contok.empty()) {
            req.SetContinuationToken(contok);
        }

        auto res = s3Client->ListObjectsV2(req);
        if(!res.IsSuccess()) {
            spdlog::error("failed to list objects at: {}: {}", path, res.GetError().GetMessage());
            return -EIO;
        }

        Aws::Vector<Aws::S3::Model::Object> objs = res.GetResult().GetContents();
        for (const auto& obj : objs) {
            struct stat stbuf;
            memset(&stbuf, 0, sizeof(stbuf));

            stbuf.st_mode = S_IFREG | 0644; // TODO support dirs
            stbuf.st_nlink = 1;
            stbuf.st_size = obj.GetSize();
            stbuf.st_mtime = obj.GetLastModified().SecondsWithMSPrecision();
            stbuf.st_uid = 0; 
            stbuf.st_gid = 0; 

            filler(buf, obj.GetKey().c_str(), &stbuf, 0, FUSE_FILL_DIR_PLUS);
        }

        contok = res.GetResult().GetNextContinuationToken();
    } while(!contok.empty());

    return 0;
} 

int object_read(
    const char *path, 
    char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    spdlog::debug("`_read` called for: {}, size: {}, offset: {}", path, size, offset);
    if(!s3Client) {
        spdlog::error("client uninitialized");
        return -EIO;
    }

    path++;

    Aws::S3::Model::GetObjectRequest req;
    req.WithBucket(BUCKET).WithKey(path);

    std::ostringstream range;
    range << "bytes=" << offset << "-" << (offset + size - 1);
    req.SetRange(range.str());

    auto res = s3Client->GetObject(req);
    if (!res.IsSuccess()) {
        spdlog::error("failed to read object at: {}: {}", path, res.GetError().GetMessage()); // TODO
        return -EIO;
    }

    auto &stream = res.GetResult().GetBody();
    stream.read(buf, size);
    auto bytesread = stream.gcount();

    return bytesread;
}

// static int object_mkdir(
//     const char *path, 
//     mode_t mode
// ) {
//     spdlog::debug("`object_mkdir` called for: %s\n", path);

//     return 0;
// }

// static int object_rmdir(const char *path) {
//     spdlog::debug("`object_rmdir` called for: %s\n", path);

//     return 0;
// }

// static int object_unlink(const char *path) {
//     spdlog::debug("`object_unlink` called for: %s\n", path);

//     return 0;
// }

// static int object_write(
//     const char *path, 
//     const char *buf, 
//     size_t size, 
//     off_t offset, 
//     struct fuse_file_info *fi
// ) {
//     spdlog::debug("`object_write` called for: %s\n", path);

//     return size;
// }

static struct fuse_operations ops = {
    .getattr = object_getattr,
    .mknod = object_mknod,
    .unlink = object_unlink,
    .read = object_read,
    .write = object_write,
    .readdir = object_readdir,
    // .mkdir = object_mkdir,
    // .rmdir = object_rmdir,
};

int main(int argc, char *argv[]) {
    spdlog::set_level(spdlog::level::debug);

    Aws::SDKOptions awsopts;
    awsopts.loggingOptions.logLevel = Aws::Utils::Logging::LogLevel::Debug;

    Aws::InitAPI(awsopts);

    Aws::Client::ClientConfiguration awsconf;
    s3Client = new Aws::S3::S3Client(awsconf);

    int res = fuse_main(argc, argv, &ops, nullptr);

    delete s3Client;
    Aws::ShutdownAPI(awsopts);

    return res;
}
