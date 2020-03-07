use crate::{ArgumentReflection, ReturnTypeReflection, Runtime, RuntimeBuilder, StructRef};
use mun_compiler::{ColorChoice, Config, Driver, FileId, PathOrInline, RelativePathBuf};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

/// Implements a compiler and runtime in one that can invoke functions. Use of the TestDriver
/// enables quick testing of Mun constructs in the runtime with hot-reloading support.
struct TestDriver {
    _temp_dir: tempfile::TempDir,
    out_path: PathBuf,
    file_id: FileId,
    driver: Driver,
    runtime: Rc<RefCell<Runtime>>,
}

impl TestDriver {
    /// Construct a new TestDriver from a single Mun source
    fn new(text: &str) -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config = Config {
            out_dir: Some(temp_dir.path().to_path_buf()),
            ..Config::default()
        };
        let input = PathOrInline::Inline {
            rel_path: RelativePathBuf::from("main.mun"),
            contents: text.to_owned(),
        };
        let (driver, file_id) = Driver::with_file(config, input).unwrap();
        let mut err_stream = mun_compiler::StandardStream::stderr(ColorChoice::Auto);
        if driver.emit_diagnostics(&mut err_stream).unwrap() {
            panic!("compiler errors..")
        }
        let out_path = driver.write_assembly(file_id).unwrap().unwrap();
        let runtime = RuntimeBuilder::new(&out_path).spawn().unwrap();
        TestDriver {
            _temp_dir: temp_dir,
            driver,
            out_path,
            file_id,
            runtime: Rc::new(RefCell::new(runtime)),
        }
    }

    /// Updates the text of the Mun source and ensures that the generated assembly has been reloaded.
    fn update(&mut self, text: &str) {
        self.driver.set_file_text(self.file_id, text);
        let out_path = self.driver.write_assembly(self.file_id).unwrap().unwrap();
        assert_eq!(
            &out_path, &self.out_path,
            "recompiling did not result in the same assembly"
        );
        let start_time = std::time::Instant::now();
        while !self.runtime.borrow_mut().update() {
            let now = std::time::Instant::now();
            if now - start_time > std::time::Duration::from_secs(10) {
                panic!("runtime did not update after recompilation within 10secs");
            } else {
                sleep(Duration::from_millis(1));
            }
        }
    }
}

macro_rules! assert_invoke_eq {
    ($ExpectedType:ty, $ExpectedResult:expr, $Driver:expr, $($Arg:tt)+) => {
        let result: $ExpectedType = invoke_fn!($Driver.runtime, $($Arg)*).unwrap();
        assert_eq!(result, $ExpectedResult, "{} == {:?}", stringify!(invoke_fn!($Driver.runtime_mut(), $($Arg)*).unwrap()), $ExpectedResult);
    }
}

#[test]
fn compile_and_run() {
    let driver = TestDriver::new(
        r"
        pub fn main() {}
    ",
    );
    assert_invoke_eq!((), (), driver, "main");
}

#[test]
fn return_value() {
    let driver = TestDriver::new(
        r"
        pub fn main():int { 3 }
    ",
    );
    assert_invoke_eq!(i64, 3, driver, "main");
}

#[test]
fn arguments() {
    let driver = TestDriver::new(
        r"
        pub fn main(a:int, b:int):int { a+b }
    ",
    );
    let a: i64 = 52;
    let b: i64 = 746;
    assert_invoke_eq!(i64, a + b, driver, "main", a, b);
}

#[test]
fn dispatch_table() {
    let driver = TestDriver::new(
        r"
        pub fn add(a:int, b:int):int { a+b }
        pub fn main(a:int, b:int):int { add(a,b) }
    ",
    );

    let a: i64 = 52;
    let b: i64 = 746;
    assert_invoke_eq!(i64, a + b, driver, "main", a, b);

    let a: i64 = 6274;
    let b: i64 = 72;
    assert_invoke_eq!(i64, a + b, driver, "add", a, b);
}

#[test]
fn booleans() {
    let driver = TestDriver::new(
        r#"
        pub fn equal(a:int, b:int):bool                 { a==b }
        pub fn equalf(a:float, b:float):bool            { a==b }
        pub fn not_equal(a:int, b:int):bool             { a!=b }
        pub fn not_equalf(a:float, b:float):bool        { a!=b }
        pub fn less(a:int, b:int):bool                  { a<b }
        pub fn lessf(a:float, b:float):bool             { a<b }
        pub fn greater(a:int, b:int):bool               { a>b }
        pub fn greaterf(a:float, b:float):bool          { a>b }
        pub fn less_equal(a:int, b:int):bool            { a<=b }
        pub fn less_equalf(a:float, b:float):bool       { a<=b }
        pub fn greater_equal(a:int, b:int):bool         { a>=b }
        pub fn greater_equalf(a:float, b:float):bool    { a>=b }
    "#,
    );
    assert_invoke_eq!(bool, false, driver, "equal", 52i64, 764i64);
    assert_invoke_eq!(bool, true, driver, "equal", 64i64, 64i64);
    assert_invoke_eq!(bool, false, driver, "equalf", 52f64, 764f64);
    assert_invoke_eq!(bool, true, driver, "equalf", 64f64, 64f64);
    assert_invoke_eq!(bool, true, driver, "not_equal", 52i64, 764i64);
    assert_invoke_eq!(bool, false, driver, "not_equal", 64i64, 64i64);
    assert_invoke_eq!(bool, true, driver, "not_equalf", 52f64, 764f64);
    assert_invoke_eq!(bool, false, driver, "not_equalf", 64f64, 64f64);
    assert_invoke_eq!(bool, true, driver, "less", 52i64, 764i64);
    assert_invoke_eq!(bool, false, driver, "less", 64i64, 64i64);
    assert_invoke_eq!(bool, true, driver, "lessf", 52f64, 764f64);
    assert_invoke_eq!(bool, false, driver, "lessf", 64f64, 64f64);
    assert_invoke_eq!(bool, false, driver, "greater", 52i64, 764i64);
    assert_invoke_eq!(bool, false, driver, "greater", 64i64, 64i64);
    assert_invoke_eq!(bool, false, driver, "greaterf", 52f64, 764f64);
    assert_invoke_eq!(bool, false, driver, "greaterf", 64f64, 64f64);
    assert_invoke_eq!(bool, true, driver, "less_equal", 52i64, 764i64);
    assert_invoke_eq!(bool, true, driver, "less_equal", 64i64, 64i64);
    assert_invoke_eq!(bool, true, driver, "less_equalf", 52f64, 764f64);
    assert_invoke_eq!(bool, true, driver, "less_equalf", 64f64, 64f64);
    assert_invoke_eq!(bool, false, driver, "greater_equal", 52i64, 764i64);
    assert_invoke_eq!(bool, true, driver, "greater_equal", 64i64, 64i64);
    assert_invoke_eq!(bool, false, driver, "greater_equalf", 52f64, 764f64);
    assert_invoke_eq!(bool, true, driver, "greater_equalf", 64f64, 64f64);
}

#[test]
fn fibonacci() {
    let driver = TestDriver::new(
        r#"
    pub fn fibonacci(n:int):int {
        if n <= 1 {
            n
        } else {
            fibonacci(n-1) + fibonacci(n-2)
        }
    }
    "#,
    );

    assert_invoke_eq!(i64, 5, driver, "fibonacci", 5i64);
    assert_invoke_eq!(i64, 89, driver, "fibonacci", 11i64);
    assert_invoke_eq!(i64, 987, driver, "fibonacci", 16i64);
}

#[test]
fn fibonacci_loop() {
    let driver = TestDriver::new(
        r#"
    pub fn fibonacci(n:int):int {
        let a = 0;
        let b = 1;
        let i = 1;
        loop {
            if i > n {
                return a
            }
            let sum = a + b;
            a = b;
            b = sum;
            i += 1;
        }
    }
    "#,
    );

    assert_invoke_eq!(i64, 5, driver, "fibonacci", 5i64);
    assert_invoke_eq!(i64, 89, driver, "fibonacci", 11i64);
    assert_invoke_eq!(i64, 987, driver, "fibonacci", 16i64);
    assert_invoke_eq!(i64, 46368, driver, "fibonacci", 24i64);
}

#[test]
fn fibonacci_loop_break() {
    let driver = TestDriver::new(
        r#"
    pub fn fibonacci(n:int):int {
        let a = 0;
        let b = 1;
        let i = 1;
        loop {
            if i > n {
                break a;
            }
            let sum = a + b;
            a = b;
            b = sum;
            i += 1;
        }
    }
    "#,
    );

    assert_invoke_eq!(i64, 5, driver, "fibonacci", 5i64);
    assert_invoke_eq!(i64, 89, driver, "fibonacci", 11i64);
    assert_invoke_eq!(i64, 987, driver, "fibonacci", 16i64);
    assert_invoke_eq!(i64, 46368, driver, "fibonacci", 24i64);
}

#[test]
fn fibonacci_while() {
    let driver = TestDriver::new(
        r#"
    pub fn fibonacci(n:int):int {
        let a = 0;
        let b = 1;
        let i = 1;
        while i <= n {
            let sum = a + b;
            a = b;
            b = sum;
            i += 1;
        }
        a
    }
    "#,
    );

    assert_invoke_eq!(i64, 5, driver, "fibonacci", 5i64);
    assert_invoke_eq!(i64, 89, driver, "fibonacci", 11i64);
    assert_invoke_eq!(i64, 987, driver, "fibonacci", 16i64);
    assert_invoke_eq!(i64, 46368, driver, "fibonacci", 24i64);
}

#[test]
fn true_is_true() {
    let driver = TestDriver::new(
        r#"
    pub fn test_true():bool {
        true
    }

    pub fn test_false():bool {
        false
    }
    "#,
    );
    assert_invoke_eq!(bool, true, driver, "test_true");
    assert_invoke_eq!(bool, false, driver, "test_false");
}

#[test]
fn hotreloadable() {
    let mut driver = TestDriver::new(
        r"
    pub fn main():int { 5 }
    ",
    );
    assert_invoke_eq!(i64, 5, driver, "main");
    driver.update(
        r"
    pub fn main():int { 10 }
    ",
    );
    assert_invoke_eq!(i64, 10, driver, "main");
}

#[test]
fn compiler_valid_utf8() {
    use std::ffi::CStr;
    use std::slice;

    let driver = TestDriver::new(
        r#"
    struct Foo {
        a: int,
    }

    pub fn foo(n:Foo):bool { false }
    "#,
    );

    let borrowed = driver.runtime.borrow();
    let foo_func = borrowed.get_function_info("foo").unwrap();
    assert_eq!(
        unsafe { CStr::from_ptr(foo_func.signature.name) }
            .to_str()
            .is_ok(),
        true
    );

    for arg_type in foo_func.signature.arg_types() {
        assert_eq!(
            unsafe { CStr::from_ptr(arg_type.name) }.to_str().is_ok(),
            true
        );

        if let Some(s) = arg_type.as_struct() {
            assert_eq!(unsafe { CStr::from_ptr(s.name) }.to_str().is_ok(), true);

            let field_names =
                unsafe { slice::from_raw_parts(s.field_names, s.num_fields as usize) };

            for field_name in field_names {
                assert_eq!(
                    unsafe { CStr::from_ptr(*field_name) }.to_str().is_ok(),
                    true
                );
            }
        }
    }
    assert_eq!(
        unsafe { CStr::from_ptr((*foo_func.signature.return_type).name) }
            .to_str()
            .is_ok(),
        true
    );
}

#[test]
fn fields() {
    let driver = TestDriver::new(
        r#"
        struct(gc) Foo { a:int, b:int };
        pub fn main(foo:int):bool {
            let a = Foo { a: foo, b: foo };
            a.a += a.b;
            let result = a;
            result.a += a.b;
            result.a == a.a
        }
    "#,
    );
    assert_invoke_eq!(bool, true, driver, "main", 48);
}

#[test]
fn field_crash() {
    let driver = TestDriver::new(
        r#"
    struct(gc) Foo { a: int };

    pub fn main(c:int):int {
        let b = Foo { a: c + 5 }
        b.a
    }
    "#,
    );
    assert_invoke_eq!(i64, 15, driver, "main", 10);
}

#[test]
fn marshal_struct() {
    let driver = TestDriver::new(
        r#"
    struct(value) Foo { a: int, b: bool };
    struct Bar(int, bool);
    struct(value) Baz(Foo);
    struct(gc) Qux(Bar);

    pub fn foo_new(a: int, b: bool): Foo {
        Foo { a, b, }
    }
    pub fn bar_new(a: int, b: bool): Bar {
        Bar(a, b)
    }
    pub fn baz_new(foo: Foo): Baz {
        Baz(foo)
    }
    pub fn qux_new(bar: Bar): Qux {
        Qux(bar)
    }
    "#,
    );

    struct TestData<T>(T, T);

    fn test_field<
        T: Copy + std::fmt::Debug + PartialEq + ArgumentReflection + ReturnTypeReflection,
    >(
        s: &mut StructRef,
        data: &TestData<T>,
        field_name: &str,
    ) {
        assert_eq!(Ok(data.0), s.get::<T>(field_name));
        s.set(field_name, data.1).unwrap();
        assert_eq!(Ok(data.1), s.replace(field_name, data.0));
        assert_eq!(Ok(data.0), s.get::<T>(field_name));
    }

    let int_data = TestData(3i64, 6i64);
    let bool_data = TestData(true, false);

    // Verify that struct marshalling works for fundamental types
    let mut foo: StructRef =
        invoke_fn!(driver.runtime, "foo_new", int_data.0, bool_data.0).unwrap();
    test_field(&mut foo, &int_data, "a");
    test_field(&mut foo, &bool_data, "b");

    let mut bar: StructRef =
        invoke_fn!(driver.runtime, "bar_new", int_data.0, bool_data.0).unwrap();
    test_field(&mut bar, &int_data, "0");
    test_field(&mut bar, &bool_data, "1");

    fn test_struct(s: &mut StructRef, c1: StructRef, c2: StructRef) {
        let field_names: Vec<String> = c1.info().field_names().map(|n| n.to_string()).collect();

        let int_value = c2.get::<i64>(&field_names[0]);
        let bool_value = c2.get::<bool>(&field_names[1]);
        s.set("0", c2).unwrap();

        let c2 = s.get::<StructRef>("0").unwrap();
        assert_eq!(c2.get::<i64>(&field_names[0]), int_value);
        assert_eq!(c2.get::<bool>(&field_names[1]), bool_value);

        let int_value = c1.get::<i64>(&field_names[0]);
        let bool_value = c1.get::<bool>(&field_names[1]);
        s.replace("0", c1).unwrap();

        let c1 = s.get::<StructRef>("0").unwrap();
        assert_eq!(c1.get::<i64>(&field_names[0]), int_value);
        assert_eq!(c1.get::<bool>(&field_names[1]), bool_value);
    }

    // Verify that struct marshalling works for struct types
    let mut baz: StructRef = invoke_fn!(driver.runtime, "baz_new", foo).unwrap();
    let c1: StructRef = invoke_fn!(driver.runtime, "foo_new", int_data.0, bool_data.0).unwrap();
    let c2: StructRef = invoke_fn!(driver.runtime, "foo_new", int_data.1, bool_data.1).unwrap();
    test_struct(&mut baz, c1, c2);

    let mut qux: StructRef = invoke_fn!(driver.runtime, "qux_new", bar).unwrap();
    let c1: StructRef = invoke_fn!(driver.runtime, "bar_new", int_data.0, bool_data.0).unwrap();
    let c2: StructRef = invoke_fn!(driver.runtime, "bar_new", int_data.1, bool_data.1).unwrap();
    test_struct(&mut qux, c1, c2);

    fn test_shallow_copy<
        T: Copy + std::fmt::Debug + PartialEq + ArgumentReflection + ReturnTypeReflection,
    >(
        s1: &mut StructRef,
        s2: &StructRef,
        data: &TestData<T>,
        field_name: &str,
    ) {
        assert_eq!(s1.get::<T>(field_name), s2.get::<T>(field_name));
        s1.set(field_name, data.1).unwrap();
        assert_ne!(s1.get::<T>(field_name), s2.get::<T>(field_name));
        s1.replace(field_name, data.0).unwrap();
        assert_eq!(s1.get::<T>(field_name), s2.get::<T>(field_name));
    }

    // Verify that StructRef::get makes a shallow copy of a struct
    let mut foo = baz.get::<StructRef>("0").unwrap();
    let foo2 = baz.get::<StructRef>("0").unwrap();
    test_shallow_copy(&mut foo, &foo2, &int_data, "a");
    test_shallow_copy(&mut foo, &foo2, &bool_data, "b");

    let mut bar = qux.get::<StructRef>("0").unwrap();
    let bar2 = qux.get::<StructRef>("0").unwrap();
    test_shallow_copy(&mut bar, &bar2, &int_data, "0");
    test_shallow_copy(&mut bar, &bar2, &bool_data, "1");

    // Specify invalid return type
    let bar_err = bar.get::<f64>("0");
    assert!(bar_err.is_err());

    // Specify invalid argument type
    let bar_err = bar.replace("0", 1f64);
    assert!(bar_err.is_err());

    // Specify invalid argument type
    let bar_err = bar.set("0", 1f64);
    assert!(bar_err.is_err());

    // Specify invalid return type
    let bar_err: Result<i64, _> = invoke_fn!(driver.runtime, "baz_new", foo);
    assert!(bar_err.is_err());

    // Pass invalid struct type
    let bar_err: Result<StructRef, _> = invoke_fn!(driver.runtime, "baz_new", bar);
    assert!(bar_err.is_err());
}

#[test]
fn hotreload_struct_decl() {
    let mut driver = TestDriver::new(
        r#"
    struct(gc) Args {
        n: int,
        foo: Bar,
    }
    
    struct(gc) Bar {
        m: float,
    }

    pub fn args(): Args {
        Args { n: 3, foo: Bar { m: 1.0 }, }
    }
    "#,
    );
    driver.update(
        r#"
    struct(gc) Args {
        n: int,
        foo: Bar,
    }
    
    struct(gc) Bar {
        m: int,
    }

    pub fn args(): Args {
        Args { n: 3, foo: Bar { m: 1 }, }
    }
    "#,
    );
}
