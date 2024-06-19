set(CMAKE_CXX_COMPILER "/nix/store/khkhbch4p1wjfl1g89gw1mszvvr7bzv0-gcc-wrapper-13.2.0/bin/g++")
set(CMAKE_CXX_COMPILER_ARG1 "")
set(CMAKE_CXX_COMPILER_ID "GNU")
set(CMAKE_CXX_COMPILER_VERSION "13.2.0")
set(CMAKE_CXX_COMPILER_VERSION_INTERNAL "")
set(CMAKE_CXX_COMPILER_WRAPPER "")
set(CMAKE_CXX_STANDARD_COMPUTED_DEFAULT "17")
set(CMAKE_CXX_EXTENSIONS_COMPUTED_DEFAULT "ON")
set(CMAKE_CXX_COMPILE_FEATURES "cxx_std_98;cxx_template_template_parameters;cxx_std_11;cxx_alias_templates;cxx_alignas;cxx_alignof;cxx_attributes;cxx_auto_type;cxx_constexpr;cxx_decltype;cxx_decltype_incomplete_return_types;cxx_default_function_template_args;cxx_defaulted_functions;cxx_defaulted_move_initializers;cxx_delegating_constructors;cxx_deleted_functions;cxx_enum_forward_declarations;cxx_explicit_conversions;cxx_extended_friend_declarations;cxx_extern_templates;cxx_final;cxx_func_identifier;cxx_generalized_initializers;cxx_inheriting_constructors;cxx_inline_namespaces;cxx_lambdas;cxx_local_type_template_args;cxx_long_long_type;cxx_noexcept;cxx_nonstatic_member_init;cxx_nullptr;cxx_override;cxx_range_for;cxx_raw_string_literals;cxx_reference_qualified_functions;cxx_right_angle_brackets;cxx_rvalue_references;cxx_sizeof_member;cxx_static_assert;cxx_strong_enums;cxx_thread_local;cxx_trailing_return_types;cxx_unicode_literals;cxx_uniform_initialization;cxx_unrestricted_unions;cxx_user_literals;cxx_variadic_macros;cxx_variadic_templates;cxx_std_14;cxx_aggregate_default_initializers;cxx_attribute_deprecated;cxx_binary_literals;cxx_contextual_conversions;cxx_decltype_auto;cxx_digit_separators;cxx_generic_lambdas;cxx_lambda_init_captures;cxx_relaxed_constexpr;cxx_return_type_deduction;cxx_variable_templates;cxx_std_17;cxx_std_20;cxx_std_23")
set(CMAKE_CXX98_COMPILE_FEATURES "cxx_std_98;cxx_template_template_parameters")
set(CMAKE_CXX11_COMPILE_FEATURES "cxx_std_11;cxx_alias_templates;cxx_alignas;cxx_alignof;cxx_attributes;cxx_auto_type;cxx_constexpr;cxx_decltype;cxx_decltype_incomplete_return_types;cxx_default_function_template_args;cxx_defaulted_functions;cxx_defaulted_move_initializers;cxx_delegating_constructors;cxx_deleted_functions;cxx_enum_forward_declarations;cxx_explicit_conversions;cxx_extended_friend_declarations;cxx_extern_templates;cxx_final;cxx_func_identifier;cxx_generalized_initializers;cxx_inheriting_constructors;cxx_inline_namespaces;cxx_lambdas;cxx_local_type_template_args;cxx_long_long_type;cxx_noexcept;cxx_nonstatic_member_init;cxx_nullptr;cxx_override;cxx_range_for;cxx_raw_string_literals;cxx_reference_qualified_functions;cxx_right_angle_brackets;cxx_rvalue_references;cxx_sizeof_member;cxx_static_assert;cxx_strong_enums;cxx_thread_local;cxx_trailing_return_types;cxx_unicode_literals;cxx_uniform_initialization;cxx_unrestricted_unions;cxx_user_literals;cxx_variadic_macros;cxx_variadic_templates")
set(CMAKE_CXX14_COMPILE_FEATURES "cxx_std_14;cxx_aggregate_default_initializers;cxx_attribute_deprecated;cxx_binary_literals;cxx_contextual_conversions;cxx_decltype_auto;cxx_digit_separators;cxx_generic_lambdas;cxx_lambda_init_captures;cxx_relaxed_constexpr;cxx_return_type_deduction;cxx_variable_templates")
set(CMAKE_CXX17_COMPILE_FEATURES "cxx_std_17")
set(CMAKE_CXX20_COMPILE_FEATURES "cxx_std_20")
set(CMAKE_CXX23_COMPILE_FEATURES "cxx_std_23")

set(CMAKE_CXX_PLATFORM_ID "Linux")
set(CMAKE_CXX_SIMULATE_ID "")
set(CMAKE_CXX_COMPILER_FRONTEND_VARIANT "GNU")
set(CMAKE_CXX_SIMULATE_VERSION "")




set(CMAKE_AR "/nix/store/khkhbch4p1wjfl1g89gw1mszvvr7bzv0-gcc-wrapper-13.2.0/bin/ar")
set(CMAKE_CXX_COMPILER_AR "/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/bin/gcc-ar")
set(CMAKE_RANLIB "/nix/store/khkhbch4p1wjfl1g89gw1mszvvr7bzv0-gcc-wrapper-13.2.0/bin/ranlib")
set(CMAKE_CXX_COMPILER_RANLIB "/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/bin/gcc-ranlib")
set(CMAKE_LINKER "/nix/store/khkhbch4p1wjfl1g89gw1mszvvr7bzv0-gcc-wrapper-13.2.0/bin/ld")
set(CMAKE_MT "")
set(CMAKE_TAPI "CMAKE_TAPI-NOTFOUND")
set(CMAKE_COMPILER_IS_GNUCXX 1)
set(CMAKE_CXX_COMPILER_LOADED 1)
set(CMAKE_CXX_COMPILER_WORKS TRUE)
set(CMAKE_CXX_ABI_COMPILED TRUE)

set(CMAKE_CXX_COMPILER_ENV_VAR "CXX")

set(CMAKE_CXX_COMPILER_ID_RUN 1)
set(CMAKE_CXX_SOURCE_FILE_EXTENSIONS C;M;c++;cc;cpp;cxx;m;mm;mpp;CPP;ixx;cppm;ccm;cxxm;c++m)
set(CMAKE_CXX_IGNORE_EXTENSIONS inl;h;hpp;HPP;H;o;O;obj;OBJ;def;DEF;rc;RC)

foreach (lang C OBJC OBJCXX)
  if (CMAKE_${lang}_COMPILER_ID_RUN)
    foreach(extension IN LISTS CMAKE_${lang}_SOURCE_FILE_EXTENSIONS)
      list(REMOVE_ITEM CMAKE_CXX_SOURCE_FILE_EXTENSIONS ${extension})
    endforeach()
  endif()
endforeach()

set(CMAKE_CXX_LINKER_PREFERENCE 30)
set(CMAKE_CXX_LINKER_PREFERENCE_PROPAGATES 1)
set(CMAKE_CXX_LINKER_DEPFILE_SUPPORTED TRUE)

# Save compiler ABI information.
set(CMAKE_CXX_SIZEOF_DATA_PTR "8")
set(CMAKE_CXX_COMPILER_ABI "ELF")
set(CMAKE_CXX_BYTE_ORDER "LITTLE_ENDIAN")
set(CMAKE_CXX_LIBRARY_ARCHITECTURE "")

if(CMAKE_CXX_SIZEOF_DATA_PTR)
  set(CMAKE_SIZEOF_VOID_P "${CMAKE_CXX_SIZEOF_DATA_PTR}")
endif()

if(CMAKE_CXX_COMPILER_ABI)
  set(CMAKE_INTERNAL_PLATFORM_ABI "${CMAKE_CXX_COMPILER_ABI}")
endif()

if(CMAKE_CXX_LIBRARY_ARCHITECTURE)
  set(CMAKE_LIBRARY_ARCHITECTURE "")
endif()

set(CMAKE_CXX_CL_SHOWINCLUDES_PREFIX "")
if(CMAKE_CXX_CL_SHOWINCLUDES_PREFIX)
  set(CMAKE_CL_SHOWINCLUDES_PREFIX "${CMAKE_CXX_CL_SHOWINCLUDES_PREFIX}")
endif()





set(CMAKE_CXX_IMPLICIT_INCLUDE_DIRECTORIES "/nix/store/3bqj2qk67b2b5wcrng8yavz3aqfhan99-libGL-1.7.0-dev/include;/nix/store/hkcrf40q709k5xjmibnyd1655rzr69rk-libxkbcommon-1.5.0-dev/include;/nix/store/w8fnbncn09blpkb1jd03dv9j2k7zmihi-systemd-minimal-libs-255.2-dev/include;/nix/store/j7yb0yjmrmjx7a8lyh8yqniywm0l1jwr-openssl-3.0.12-dev/include;/nix/store/vxykhyxbc8pbc85yc4k3jqfv2j5k4ccp-wayland-1.22.0-dev/include;/nix/store/77gsvp9449ydvnz2j91dyvh8d58gdwg7-libX11-1.8.7-dev/include;/nix/store/nymgy9hw32mavimh1ghnw0s8nkwsg71k-xorgproto-2023.2/include;/nix/store/a03irllzpb61zwcrm7hp5v2n0mxlxpg3-libxcb-1.16-dev/include;/nix/store/jyxfkrvcf9b5rlb0x2qp22sjw1yjnzjb-libXcursor-1.2.1-dev/include;/nix/store/ikjksvw2fis8jgc6ry0mwi8fc36ahnh7-libXi-1.8.1-dev/include;/nix/store/2g5x8plq5gk8khg7l7hjyk58ljayh14r-libXfixes-6.0.1-dev/include;/nix/store/shkyhxd90rs8l9c7c7fvzxqhr6h353qs-libXext-1.3.5-dev/include;/nix/store/mcgbrihx6mxv6ppgf5qnfcxk6j2179v0-libXau-1.0.11-dev/include;/nix/store/8hbjc95q7d6d4yacrc84zs00wqljz3nk-libXrandr-1.5.4-dev/include;/nix/store/5jf4wjcyk7gzx6241q02cw6f8h7vpp93-libXrender-0.9.11-dev/include;/nix/store/c69j333svfnf70jawx94v17zd9sw20ln-dbus-1.14.10-dev/include;/nix/store/gmq2vvrj0a2h7jq9kjjdx33hz7v4z95w-expat-2.5.0-dev/include;/nix/store/5i5l2m4ghzwsbxnlv2gvk0ppdca4z62m-jq-1.7.1-dev/include;/nix/store/0pb53w0y40lm7imb9a9y10hddai7yabk-zstd-1.5.5-dev/include;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/include/c++/13.2.0;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/include/c++/13.2.0/x86_64-unknown-linux-gnu;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/include/c++/13.2.0/backward;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/lib/gcc/x86_64-unknown-linux-gnu/13.2.0/include;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/include;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/lib/gcc/x86_64-unknown-linux-gnu/13.2.0/include-fixed;/nix/store/3mmvgb08qy8n6n37mnprf77fnp4rssi9-glibc-2.38-27-dev/include")
set(CMAKE_CXX_IMPLICIT_LINK_LIBRARIES "stdc++;m;gcc_s;gcc;c;gcc_s;gcc")
set(CMAKE_CXX_IMPLICIT_LINK_DIRECTORIES "/nix/store/g2k6pslg3dy4jn2hnz3aahsjvdbfyfgv-libGL-1.7.0/lib;/nix/store/k203rr7im6rhbqpdravvzp92y2fn9mkn-libglvnd-1.7.0/lib;/nix/store/pav3ikskqx8nzynjr09pnd4js0v0hanp-libxkbcommon-1.5.0/lib;/nix/store/p29wd6v1nr9c11w58x45pcykrllc88rn-systemd-minimal-libs-255.2/lib;/nix/store/1l3a02nqq5b5v7rhchj89hi7plmbza5r-openssl-3.0.12/lib;/nix/store/5jkv5a5v8iivbjb0b3948wf3hqsacs1f-wayland-1.22.0/lib;/nix/store/d34xzgg5adx5l4ps79ppyb98lvyhkgm5-libxcb-1.16/lib;/nix/store/x51ly05chwj47xgz5grn48rz5k2mvzlg-libX11-1.8.7/lib;/nix/store/nk9kvh3f3krddzl6wr5szbg3ydpx8l90-libXcursor-1.2.1/lib;/nix/store/91i5azhpqrbhggpkrh7z23plh47gg8qi-libXfixes-6.0.1/lib;/nix/store/y83n31linby6r3j74qmrx07p4j3cvn3n-libXau-1.0.11/lib;/nix/store/8a30syk0pipph0m1baz281as60q2d33m-libXext-1.3.5/lib;/nix/store/0r4jvkv443slsbw4qw0nhfk7hmkprv51-libXi-1.8.1/lib;/nix/store/f3gn7hqahmqgk9pxhl85irmbn1jjhswv-libXrender-0.9.11/lib;/nix/store/kk23svq1dfg4hd2jrb8dxhbdqla66acr-libXrandr-1.5.4/lib;/nix/store/8ah0ykrhnb24rppms5dp8nzg1z9n8r40-expat-2.5.0/lib;/nix/store/vw8rfrcf4rqc72a5z6lzy4sgn7p6c11z-dbus-1.14.10-lib/lib;/nix/store/x0v2bqb81qfabk8nbmc4wjmnfpz6sglb-jq-1.7.1-lib/lib;/nix/store/c5gp1vii3qmma428db3gyyb91p710whs-zstd-1.5.5/lib;/nix/store/7jiqcrg061xi5clniy7z5pvkc4jiaqav-glibc-2.38-27/lib;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/lib/gcc/x86_64-unknown-linux-gnu/13.2.0;/nix/store/np3cndfk53miqg2cilv7vfdxckga665h-gcc-13.2.0-lib/lib;/nix/store/khkhbch4p1wjfl1g89gw1mszvvr7bzv0-gcc-wrapper-13.2.0/bin;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/lib64;/nix/store/j00nb8s5mwaxgi77h21i1ycb91yxxqck-gcc-13.2.0/lib")
set(CMAKE_CXX_IMPLICIT_LINK_FRAMEWORK_DIRECTORIES "")
