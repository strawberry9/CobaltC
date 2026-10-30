/* A fixture of extern_code_asm_file_ok.cb: two functions in assembly,
   `addmul(a, b, c) = a * b + c` on u64 and `seven()`. The file is
   preprocessed (.S), so the platform's calling convention is chosen here:
   System V passes in %rdi, %rsi, %rdx; Windows x64 in %rcx, %rdx, %r8. */
        .text
#ifdef _WIN32
        .globl  addmul
addmul:
        movq    %rcx, %rax
        imulq   %rdx, %rax
        addq    %r8, %rax
        ret
#else
        .globl  addmul
        .type   addmul, @function
addmul:
        movq    %rdi, %rax
        imulq   %rsi, %rax
        addq    %rdx, %rax
        ret
        .size   addmul, .-addmul
#endif
        .globl  seven
seven:
        movl    $7, %eax
        ret
#ifndef _WIN32
        .section .note.GNU-stack,"",@progbits
#endif
