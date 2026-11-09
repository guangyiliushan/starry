$enc = New-Object System.Text.UTF8Encoding $false
$nl = "`n"
$p = 'D:\project\project\starry\starry\crates\starryc\tests\build_cli.rs'
$c = [IO.File]::ReadAllText($p)
$add = $nl
$add += '#[test]' + $nl
$add += 'fn generic_bounds_end_to_end() {' + $nl
$add += '    // M9 9c: bounded type params — the concrete type argument must' + $nl
$add += '    // implement the bound trait; trait method calls through the' + $nl
$add += '    // type parameter resolve in the instantiated clone.' + $nl
$add += '    let Some(out) = build_and_run(' + $nl
$add += '        "generic_bounds",' + $nl
$add += '        "trait Show {\n    fun show(): string\n}\nstruct Point(val x: int)\nstruct Label(val t: string)\nimpl Show for Point {\n    fun show(): string {\n        return \"pt\"\n    }\n}\nimpl Show for Label {\n    fun show(): string {\n        return \"lbl\"\n    }\n}\nfun describe<T: Show>(x: T): string {\n    return x.show()\n}\nfun main() {\n    val a = describe(Point(1))\n    val b = describe(Label(\"L\"))\n    print(a)\n    print(b)\n}\n",' + $nl
$add += '    ) else {' + $nl
$add += '        return;' + $nl
$add += '    };' + $nl
$add += '    assert_eq!(out, "pt\nlbl\n");' + $nl
$add += '}' + $nl
$c = $c + $add
[IO.File]::WriteAllText($p, $c, $enc)
'OK'