$enc = New-Object System.Text.UTF8Encoding $false
$nl = "`n"

# 1. DefSig.tp_bounds
$p = 'D:\project\project\starry\starry\crates\sem\src\def.rs'
$c = [IO.File]::ReadAllText($p)
$old = '    /// M7 slice 2：类型形参默认值（declare 期在类型形参作用域 lower；' + $nl
$old += '    /// 与 type_params 对齐；未提供的为 None）' + $nl
$old += '    pub tp_defaults: Vec<Option<TyId>>,'
$new = '    /// M7 slice 2：类型形参默认值（declare 期在类型形参作用域 lower；' + $nl
$new += '    /// 与 type_params 对齐；未提供的为 None）' + $nl
$new += '    pub tp_defaults: Vec<Option<TyId>>,' + $nl
$new += '    /// M9 9c：类型形参 bounds——每项 = 该形参的 trait def 列表' + $nl
$new += '    ///（`T : A & B` 交集；实例化时逐一查 impl 注册表兑现）' + $nl
$new += '    pub tp_bounds: Vec<Vec<DefId>>,'
if (-not $c.Contains($old)) { throw 'defsig anchor' }
$c = $c.Replace($old, $new)
$old = '            tp_defaults: Vec::new(),'
$new = '            tp_defaults: Vec::new(),' + $nl
$new += '            tp_bounds: Vec::new(),'
if (-not $c.Contains($old)) { throw 'new anchor' }
$c = $c.Replace($old, $new)
[IO.File]::WriteAllText($p, $c, $enc)

# 2. declare：fun/struct 的 bounds 解析 + 拒收面收窄
$p = 'D:\project\project\starry\starry\crates\sem\src\declare.rs'
$c = [IO.File]::ReadAllText($p)
$old = '    // M7 slice 2：类型形参默认值已落地（调用点推断的兜底）；' + $nl
$old += '    // bounds/variance/where 仍拒收（bounds 依赖 trait，M9 前）' + $nl
$old += '    let fancy = type_params' + $nl
$old += '        .iter()' + $nl
$old += '        .any(|tp| tp.variance.is_some() || !tp.bounds.is_empty());'
$new = '    // M7 slice 2 + M9 9c：默认值与 bounds 已落地；variance/where 仍拒收' + $nl
$old2 = $new
$new = '    // M7 slice 2 + M9 9c：类型形参默认值与 bounds 已落地；variance/' + $nl
$new += '    // where 仍拒收（where 是 bounds 的语法糖后置）' + $nl
$new += '    let fancy = type_params' + $nl
$new += '        .iter()' + $nl
$new += '        .any(|tp| tp.variance.is_some());'
if (-not $c.Contains($old)) { throw 'fancy anchor' }
$c = $c.Replace($old, $new)

# fun arm：tp_bounds 解析（在 tp_defaults lower 之后）
$old = '                st.defs.get_mut(def).sig.tp_defaults = tp_defaults;' + $nl
$old += '            }'
$new = '                st.defs.get_mut(def).sig.tp_defaults = tp_defaults;' + $nl
$new += '                // M9 9c：bounds 解析为 trait def 列表（tscope 内解析）' + $nl
$new += '                let tp_bounds: Vec<Vec<DefId>> = f' + $nl
$new += '                    .type_params' + $nl
$new += '                    .iter()' + $nl
$new += '                    .map(|tp| {' + $nl
$new += '                        tp.bounds' + $nl
$new += '                            .iter()' + $nl
$new += '                            .filter_map(|b| match st.tys.kind(lower_ty(st, tscope, b)) {' + $nl
$new += '                                TyKind::Named(d, _)' + $nl
$new += '                                    if matches!(st.defs.get(*d).kind, DefKind::Trait) =>' + $nl
$new += '                                {' + $nl
$new += '                                    Some(*d)' + $nl
$new += '                                }' + $nl
$new += '                                _ => {' + $nl
$new += '                                    st.diags.emit(Diagnostic::error(' + $nl
$new += '                                        DiagCode::SemTypeMismatch,' + $nl
$new += '                                        b.span,' + $nl
$new += '                                        "type parameter bounds must name traits",' + $nl
$new += '                                    ));' + $nl
$new += '                                    None' + $nl
$new += '                                }' + $nl
$new += '                            })' + $nl
$new += '                            .collect()' + $nl
$new += '                    })' + $nl
$new += '                    .collect();' + $nl
$new += '                st.defs.get_mut(def).sig.tp_bounds = tp_bounds;' + $nl
$new += '            }'
if (-not $c.Contains($old)) { throw 'fun bounds anchor' }
$c = $c.Replace($old, $new)

# struct arm：sscope 的 tp_bounds
$old = '                st.defs.get_mut(def).sig.tp_defaults = tp_defaults;' + $nl
$old += '                // M3 主构造：头 val/var 参数类型'
$new = '                st.defs.get_mut(def).sig.tp_defaults = tp_defaults;' + $nl
$new += '                // M9 9c：struct 形参 bounds（sscope 内解析）' + $nl
$new += '                let tp_bounds: Vec<Vec<DefId>> = s' + $nl
$new += '                    .type_params' + $nl
$new += '                    .iter()' + $nl
$new += '                    .map(|tp| {' + $nl
$new += '                        tp.bounds' + $nl
$new += '                            .iter()' + $nl
$new += '                            .filter_map(|b| match st.tys.kind(lower_ty(st, sscope, b)) {' + $nl
$new += '                                TyKind::Named(d, _)' + $nl
$new += '                                    if matches!(st.defs.get(*d).kind, DefKind::Trait) =>' + $nl
$new += '                                {' + $nl
$new += '                                    Some(*d)' + $nl
$new += '                                }' + $nl
$new += '                                _ => {' + $nl
$new += '                                    st.diags.emit(Diagnostic::error(' + $nl
$new += '                                        DiagCode::SemTypeMismatch,' + $nl
$new += '                                        b.span,' + $nl
$new += '                                        "type parameter bounds must name traits",' + $nl
$new += '                                    ));' + $nl
$new += '                                    None' + $nl
$new += '                                }' + $nl
$new += '                            })' + $nl
$new += '                            .collect()' + $nl
$new += '                    })' + $nl
$new += '                    .collect();' + $nl
$new += '                st.defs.get_mut(def).sig.tp_bounds = tp_bounds;' + $nl
$new += '                // M3 主构造：头 val/var 参数类型'
if (-not $c.Contains($old)) { throw 'struct bounds anchor' }
$c = $c.Replace($old, $new)
[IO.File]::WriteAllText($p, $c, $enc)
'part1 OK'