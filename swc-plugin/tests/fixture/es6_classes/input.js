// ES6 Class annotations via ngInject

(function(){
    class ClassTest1 {
        constructor($log) {}
    }
    /** @ngInject */
    class ClassTest1_noargs {
        constructor() {}
    }
    /** @ngInject */
    class ClassTest1_annotated {
        constructor($log) {}
    }
    class ClassTest1_annotated_constructor {
        /** @ngInject */
        constructor($log) {}
    }
    class ClassTest1_prologue_directive {
        constructor($log) {
            "ngInject";
        }
    }

    let ClassTest2 = class {
        constructor($log) {}
    };
    /** @ngInject */
    let ClassTest2_noargs = class {
        constructor() {}
    };
    /** @ngInject */
    let ClassTest2_annotated = class {
        constructor($log) {}
    };

    // Anonymous class expression as object property value
    var component = {
        controller: class {
            /* @ngInject */
            constructor($element, $log) {}
        }
    };
})();
