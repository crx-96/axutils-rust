//! 路径判断、获取和拼接工具。

use std::path::{Component, Path, PathBuf};

/// 路径处理工具。
#[derive(Debug, Clone, Copy, Default)]
pub struct PathUtils;

impl PathUtils {
    /// 判断路径是否为绝对路径。
    ///
    /// 该方法只根据当前平台的路径语法判断，不访问文件系统，也不会检查路径是否存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::PathUtils;
    ///
    /// let current_dir = std::env::current_dir().expect("current directory should be available");
    /// assert!(PathUtils::is_absolute(current_dir));
    /// assert!(!PathUtils::is_absolute("./var/log"));
    /// ```
    pub fn is_absolute<P>(path: P) -> bool
    where
        P: AsRef<Path>,
    {
        path.as_ref().is_absolute()
    }

    /// 获取当前进程的工作目录。
    ///
    /// 该方法直接调用操作系统接口，因此工作目录不存在、无权访问或操作系统无法返回
    /// 工作目录时会返回错误。返回值不代表对目录内容做了任何检查。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::PathUtils;
    ///
    /// let current_dir = PathUtils::current_dir().expect("current directory should be available");
    /// assert!(!current_dir.as_os_str().is_empty());
    /// ```
    pub fn current_dir() -> std::io::Result<PathBuf> {
        std::env::current_dir()
    }

    /// 获取当前进程可执行文件的路径。
    ///
    /// 该方法直接调用操作系统接口，不会主动解析符号链接，也不保证返回的路径在方法
    /// 返回后仍然指向同一个文件。操作系统无法返回可执行文件路径时会返回错误。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::PathUtils;
    ///
    /// let executable =
    ///     PathUtils::executable_path().expect("the current executable should be available");
    /// assert!(!executable.as_os_str().is_empty());
    /// ```
    pub fn executable_path() -> std::io::Result<PathBuf> {
        std::env::current_exe()
    }

    /// 按顺序拼接多个路径，并在词法层面处理 `.` 和 `..`。
    ///
    /// 路径片段通过 [`PathBuf::push`] 的平台规则依次追加；如果后续片段是绝对路径，
    /// 它会替换此前已经拼接的内容。方法不会访问文件系统，因此不会解析符号链接、挂载点
    /// 或实际存在性。有根路径根目录之外的 `..` 会被忽略；没有根目录的相对路径开头
    /// 超出当前片段的 `..` 会被保留。
    /// 输入为空时返回当前目录的词法表示 `PathBuf::from(".")`。
    ///
    /// 该方法的时间和空间开销与输入路径总长度线性相关，返回的新路径会分配与结果规模
    /// 相称的内存。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::PathUtils;
    /// use std::path::PathBuf;
    ///
    /// let path = PathUtils::join(["project", "src", "..", "./README.md"]);
    /// assert_eq!(path, PathBuf::from("project").join("README.md"));
    /// ```
    pub fn join<I, P>(paths: I) -> PathBuf
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut joined = PathBuf::new();

        for path in paths {
            joined.push(path);
        }

        Self::normalize(joined)
    }

    fn normalize(path: PathBuf) -> PathBuf {
        let has_root = path.has_root();
        let mut normalized = PathBuf::new();

        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => match normalized.components().next_back() {
                    Some(Component::Normal(_)) => {
                        normalized.pop();
                    }
                    _ if !has_root => normalized.push(".."),
                    _ => {}
                },
                component => normalized.push(component.as_os_str()),
            }
        }

        if normalized.as_os_str().is_empty() && !has_root {
            normalized.push(".");
        }

        normalized
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::PathUtils;

    #[test]
    fn detects_absolute_and_relative_paths() {
        let current_dir = std::env::current_dir().expect("current directory should be available");

        assert!(PathUtils::is_absolute(current_dir));
        assert!(!PathUtils::is_absolute(Path::new("relative/path")));
        assert!(!PathUtils::is_absolute(Path::new("./relative/path")));
    }

    #[test]
    fn returns_current_working_directory() {
        let current_dir = PathUtils::current_dir().expect("current directory should be available");

        assert!(!current_dir.as_os_str().is_empty());
        assert!(current_dir.is_absolute());
        assert_eq!(
            current_dir,
            std::env::current_dir().expect("current directory should be available")
        );
    }

    #[test]
    fn returns_current_executable_path() {
        let executable =
            PathUtils::executable_path().expect("current executable should be available");

        assert!(!executable.as_os_str().is_empty());
        assert!(executable.is_absolute());
        assert_eq!(
            executable,
            std::env::current_exe().expect("current executable should be available")
        );
    }

    #[test]
    fn joins_and_normalizes_relative_components() {
        assert_eq!(
            PathUtils::join(["project", "src", ".", "..", "README.md"]),
            PathBuf::from("project").join("README.md")
        );
    }

    #[test]
    fn preserves_relative_parent_components() {
        assert_eq!(
            PathUtils::join(["..", "..", "src"]),
            PathBuf::from("..").join("..").join("src")
        );
    }

    #[test]
    fn does_not_escape_an_absolute_root() {
        let current_dir = std::env::current_dir().expect("current directory should be available");
        let base = current_dir.join("project");
        let expected = current_dir.join("README.md");

        assert_eq!(
            PathUtils::join([
                base,
                PathBuf::from("src"),
                PathBuf::from(".."),
                PathBuf::from(".."),
                PathBuf::from("README.md")
            ]),
            expected
        );
    }

    #[cfg(windows)]
    #[test]
    fn does_not_escape_a_root_relative_path() {
        assert_eq!(PathUtils::join([r"\.."]), PathBuf::from("\\"));
        assert_eq!(
            PathUtils::join([r"\foo", r"..", r"..", r"bar"]),
            PathBuf::from(r"\bar")
        );
    }

    #[test]
    fn later_absolute_path_replaces_previous_components() {
        let current_dir = std::env::current_dir().expect("current directory should be available");
        let expected = current_dir.join("README.md");

        assert_eq!(
            PathUtils::join([
                PathBuf::from("ignored"),
                current_dir,
                PathBuf::from("README.md")
            ]),
            expected
        );
    }

    #[test]
    fn empty_input_represents_current_directory() {
        assert_eq!(
            PathUtils::join(std::iter::empty::<&str>()),
            PathBuf::from(".")
        );
    }
}
